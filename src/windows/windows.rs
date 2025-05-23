//! This uses the desktop duplication api.
//! https://docs.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api
//! Basic steps are:
//!  - Create the IDXGI adaptor
//!  - Obtain the Output
//!  - Create the duplicator
//!  - Create a texture for the duplicator to write into.
//!  - And then, when an image is requested, we map a new image to the texture the duplicator wrote.
//!
//! Whenever failure happens, we try to reinstantiate the duplicator, this can happen when the
//! resolution changes, or when we don't have permissions to do a screen capture.

use crate::*;
use windows;

use windows::core::Error as WinError;
use windows::core::Interface;
use windows::{
    Win32::Graphics::Direct3D11::*, Win32::Graphics::Dxgi::Common::*, Win32::Graphics::Dxgi::*,
};

struct ImageWin {
    _image: ID3D11Texture2D,
    mapped: windows::Win32::Graphics::Direct3D11::D3D11_MAPPED_SUBRESOURCE,
    width: u32,
    height: u32,
}

fn initialisation_error(v: WinError) -> ScreenCaptureError {
    ScreenCaptureError::InitialisationError {
        msg: format!("{v:?}"),
    }
}

fn logic_error(v: WinError) -> ScreenCaptureError {
    ScreenCaptureError::LogicError {
        msg: format!("{v:?}"),
    }
}

fn lost_capture_error(v: WinError) -> ScreenCaptureError {
    ScreenCaptureError::LostCaptureError {
        msg: format!("{v:?}"),
    }
}

trait PrintingExpect {
    type Inner;
    fn expect_with(self, msg: &'static str) -> Self::Inner;
}
impl<T> PrintingExpect for Option<T> {
    type Inner = T;
    fn expect_with(self, msg: &'static str) -> Self::Inner {
        if self.is_none() {
            println!("option expect failed: {msg}");
        }
        self.unwrap()
    }
}
impl<T, U: std::fmt::Debug> PrintingExpect for Result<T, U> {
    type Inner = T;
    fn expect_with(self, msg: &'static str) -> Self::Inner {
        if !self.is_ok() {
            println!("result expect failed: {msg}");
        }
        self.unwrap()
    }
}

impl ImageWin {
    fn new(texture: ID3D11Texture2D) -> Self {
        // Need to map the texture here to ensure we can read from it later.

        let mut desc: windows::Win32::Graphics::Direct3D11::D3D11_TEXTURE2D_DESC =
            Default::default();
        unsafe { texture.GetDesc(&mut desc) };

        let width = desc.Width;
        let height = desc.Height;

        // Map the texture, retrieval of device and context and mapping from.
        // https://github.com/Microsoft/graphics-driver-samples/blob/master/render-only-sample/rostest/util.cpp
        // Get the device, get the context, then map the texture.
        let mapped;
        unsafe {
            let mut device: Option<ID3D11Device> = None;
            texture.GetDevice(&mut device);
            let device = device.expect_with("Should have a device associated to it.");

            let mut context: Option<ID3D11DeviceContext> = None;
            device.GetImmediateContext(&mut context);
            let context = context.expect_with("Should have a context associated to it.");

            // Now that we have the context, we can perform the mapping.
            mapped = context
                .Map(
                    &texture,
                    0, // subresource
                    D3D11_MAP_READ,
                    0,
                )
                .expect_with("Mapping should succeed"); // MapFlags
        }
        ImageWin {
            width,
            height,
            _image: texture,
            mapped,
        }
    }
}

impl ImageBGR for ImageWin {
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }

    fn pixel(&self, x: u32, y: u32) -> BGR {
        if x > self.width || y > self.height {
            panic!("Retrieved out of bounds ({}, {})", x, y);
        }
        // Finally, we can now do the whole casting dance on the mappe data, and calculate what to retrieve.
        // const uint8_t* data = reinterpret_cast<const uint8_t*>(mapped_.pData);
        // const uint8_t stride = (mapped_.RowPitch / getWidth());
        // return (*reinterpret_cast<const uint32_t*>(data + y * mapped_.RowPitch + x * stride)) & 0x00FFFFFF;
        unsafe {
            // println!("Image: {:?}", self.image.unwrap());
            // Do some pointer magic and reach into the data, do a few casts and we're golden.
            let data =
                std::mem::transmute::<*const core::ffi::c_void, *const u8>(self.mapped.pData);
            let stride = (self.mapped.RowPitch / self.width) as u32;
            // println!("rowpitch {}", self.mapped.RowPitch); 7680 for 1920
            // println!("stride {}", stride); 4
            let as_integer = *std::mem::transmute::<*const u8, *const u32>(
                data.offset((y * self.mapped.RowPitch + x * stride) as isize)
                    .try_into()
                    .unwrap(),
            );
            let masked = as_integer & 0x00FFFFFF;
            // println!("as integer: {}", as_integer);
            return BGR {
                r: ((masked >> 16) & 0xFF) as u8,
                g: ((masked >> 8) & 0xFF) as u8,
                b: (masked & 0xFF) as u8,
            };
        }
    }

    fn data(&self) -> &[BGR] {
        // Should always have an image.
        unsafe {
            let data =
                std::mem::transmute::<*const core::ffi::c_void, *const BGR>(self.mapped.pData);
            let width = self.width as usize;
            let height = self.height as usize;
            let stride = (self.mapped.RowPitch / self.width) as u32;
            assert!(stride == 4);
            assert!(self.mapped.RowPitch / stride == self.width);
            let len = width * height;
            std::slice::from_raw_parts(data, len)
        }
    }
}

// For d3d12 we could follow  https://github.com/microsoft/windows-samples-rs/blob/5d67b33e7115ec1dd4f8448301bf6ce794c93b5f/direct3d12/src/main.rs#L204-L234.

#[derive(Default)]
struct CaptureWin {
    adaptor: Option<IDXGIAdapter1>,
    device: Option<ID3D11Device>,
    device_context: Option<ID3D11DeviceContext>,
    debug_device: Option<ID3D11Debug>,
    info_queue: Option<ID3D11InfoQueue>,
    output: Option<IDXGIOutput>,
    duplicator: Option<IDXGIOutputDuplication>,

    image: Option<ID3D11Texture2D>,
}

impl Drop for CaptureWin {
    fn drop(&mut self) {
        unsafe {
            // Drop the things we can actually release.
            if let Some(duplicator) = self.duplicator.as_ref() {
                let _ = duplicator.ReleaseFrame();
            }

            if let Some(output) = self.output.as_ref() {
                let _ = output.ReleaseOwnership();
            }
        }
    }
}

use std::ffi::OsString;
use std::os::windows::prelude::*;

// Apparently from_wide from OsString doesn't respect zero termination.
fn from_wide(arr: &[u16]) -> OsString {
    let len = arr.iter().take_while(|c| **c != 0).count();
    OsString::from_wide(&arr[..len])
}

impl CaptureWin {
    fn init_adaptor(&mut self) -> Result<(), ScreenCaptureError> {
        let dxgi_factory_flags = DXGI_CREATE_FACTORY_DEBUG;
        let factory: IDXGIFactory4 =
            unsafe { CreateDXGIFactory2(dxgi_factory_flags) }.map_err(initialisation_error)?;

        for i in 0.. {
            let adapter = unsafe { factory.EnumAdapters1(i).map_err(initialisation_error)? };
            let desc = unsafe { adapter.GetDesc1().map_err(initialisation_error)? };

            // Skip the software adaptor.
            if (DXGI_ADAPTER_FLAG::from(desc.Flags) & DXGI_ADAPTER_FLAG_SOFTWARE)
                != DXGI_ADAPTER_FLAG_NONE
            {
                continue;
            }

            // Print some info about the adapter.
            /*
            println!(
                "Adapter {} -> {:#?} with {} memory",
                i,
                from_wide(&desc.Description),
                desc.DedicatedVideoMemory
            );
            */

            // Instantiate the d3d11 device now.
            let sdk_version = windows::Win32::Graphics::Direct3D11::D3D11_SDK_VERSION;
            let create_flags =
                windows::Win32::Graphics::Direct3D11::D3D11_CREATE_DEVICE_BGRA_SUPPORT
                    | windows::Win32::Graphics::Direct3D11::D3D11_CREATE_DEVICE_DEBUG;
            let mut level_used = windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_9_3;
            let feature_levels = [
                windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0,
                windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_10_1,
                windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_10_0,
                windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_9_3,
            ];

            let create_device_res = unsafe {
                D3D11CreateDevice(
                    &adapter,                                                    // padapter: Param0,
                    windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN, // drivertype: D3D_DRIVER_TYPE,
                    0,            // software: Param2,
                    create_flags, // flags: D3D11_CREATE_DEVICE_FLAG,
                    &feature_levels as *const windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL, // pfeaturelevels: *const D3D_FEATURE_LEVEL,
                    feature_levels.len() as u32, // featurelevels: u32,
                    sdk_version,                 // sdkversion: u32,
                    &mut self.device,            // ppdevice: *mut Option<ID3D11Device>,
                    &mut level_used,             // pfeaturelevel: *mut D3D_FEATURE_LEVEL,
                    &mut self.device_context, // ppimmediatecontext: *mut Option<ID3D11DeviceContext>
                )
            };
            if create_device_res.is_ok() {
                self.adaptor = Some(adapter);

                return Ok(()); // we had success.
            };
        }

        Err(ScreenCaptureError::InitialisationError {
            msg: "failed to find adapter".to_owned(),
        })
    }

    fn init_debug(&mut self) -> Result<(), ScreenCaptureError> {
        // Now that we have the device, we can create the debug device and info queue.
        let debug_device: Result<ID3D11Debug, WinError> = self.device.as_ref().unwrap().cast();
        self.debug_device = Some(debug_device.map_err(initialisation_error)?);
        let info_queue: Result<ID3D11InfoQueue, WinError> =
            self.debug_device.as_ref().unwrap().cast();
        self.info_queue = Some(info_queue.map_err(initialisation_error)?);
        let info_queue = self.info_queue.as_ref().unwrap();
        unsafe {
            // Enabling this actually makes it abort if a message is encountered, can we hook up a printer?
            //info_queue.SetBreakOnSeverity(DXGI_INFO_QUEUE_MESSAGE_SEVERITY_CORRUPTION, true).map_err(initialisation_error)?;
            //info_queue.SetBreakOnSeverity(DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR, true).map_err(initialisation_error)?;
            //info_queue.SetBreakOnSeverity(DXGI_INFO_QUEUE_MESSAGE_SEVERITY_INFO, true).map_err(initialisation_error)?;
            //info_queue.SetBreakOnSeverity(DXGI_INFO_QUEUE_MESSAGE_SEVERITY_MESSAGE, true).map_err(initialisation_error)?;
            //info_queue.SetBreakOnSeverity(DXGI_INFO_QUEUE_MESSAGE_SEVERITY_WARNING, true).map_err(initialisation_error)?;
            // Add a dummy message for testing.
            if false {
                const DUMMY_DATA: [u8; 5] = [0x41, 0x42, 0x43, 0x44, 0]; // ABC\x00
                let mut_pointer_to_const_sketchy =
                    std::mem::transmute::<_, *mut u8>(DUMMY_DATA.as_ptr());
                info_queue
                    .AddMessage(
                        windows::Win32::Graphics::Dxgi::DXGI_INFO_QUEUE_MESSAGE_CATEGORY_CLEANUP,
                        windows::Win32::Graphics::Dxgi::DXGI_INFO_QUEUE_MESSAGE_SEVERITY_WARNING,
                        windows::Win32::Graphics::Direct3D11::D3D11_MESSAGE_ID_UNKNOWN,
                        windows::Win32::Foundation::PSTR(mut_pointer_to_const_sketchy),
                    )
                    .map_err(initialisation_error)?;
            }
        }
        Ok(())
    }

    pub fn get_debug_messages(&self) -> Result<Vec<String>, ScreenCaptureError> {
        if self.info_queue.is_none() {
            return Err(ScreenCaptureError::InitialisationError {
                msg: "cannot get debug message without init_debug called".to_owned(),
            });
        }
        let info_queue = self.info_queue.as_ref().unwrap();
        let mut msgs = vec![];
        unsafe {
            let current_count = info_queue.GetNumStoredMessages();
            for i in 0..current_count {
                let mut message_length: usize = 0;
                info_queue
                    .GetMessage(i, std::ptr::null_mut(), &mut message_length)
                    .map_err(lost_capture_error)?;
                if message_length == 0 {
                    break;
                }
                let mut data = vec![0; message_length];
                let msg_ptr: *mut D3D11_MESSAGE = std::mem::transmute(data.as_mut_ptr());
                info_queue
                    .GetMessage(0, msg_ptr, &mut message_length)
                    .map_err(lost_capture_error)?;
                let descr_slice = std::slice::from_raw_parts(
                    (*msg_ptr).pDescription,
                    (*msg_ptr).DescriptionByteLength,
                );
                let desc = std::ffi::CStr::from_bytes_with_nul_unchecked(descr_slice);
                let desc_str = desc.to_str().expect("should be a valid utf 8 string");
                let formatted = format!(
                    "{:?} {:?} {:?}: {}",
                    (*msg_ptr).Category,
                    (*msg_ptr).Severity,
                    (*msg_ptr).ID,
                    desc_str
                );
                msgs.push(formatted);
            }

            info_queue.ClearStoredMessages();
        }
        Ok(msgs)
    }

    fn dump_debug_messages(&self) {
        if let Ok(msgs) = self.get_debug_messages() {
            for msg in msgs {
                println!("D3D Debug Message: {msg}");
            }
        } else {
            println!("Failed to get debug messages!");
        }
    }

    fn init_output(&mut self, desired: u32) -> Result<(), ScreenCaptureError> {
        // Obtain the video outputs used by this adaptor.
        // Is the primary screen always the zeroth index??
        if self.adaptor.is_none() {
            return Err(ScreenCaptureError::InitialisationError {
                msg: "cannot prepare without valid adapter".to_owned(),
            });
        }
        let adaptor = self.adaptor.as_ref().unwrap();
        let mut output_index: u32 = 0;
        unsafe {
            let mut res = adaptor.EnumOutputs(output_index);
            while let Ok(output) = res {
                if desired == output_index {
                    /*
                    let desc = output.GetDesc().map_err(initialisation_error)?;
                    println!(
                        "Found desired output: {}, name: {}, monitor: {}",
                        output_index,
                        OsString::from_wide(&desc.DeviceName)
                            .to_str()
                            .unwrap_or("Unknown"),
                        desc.Monitor
                    );
                    */
                    self.output = Some(output);
                    return Ok(());
                }
                output_index = output_index + 1;
                res = adaptor.EnumOutputs(output_index);
            }
        }

        Err(ScreenCaptureError::InitialisationError {
            msg: "failed initialise output".to_owned(),
        })
    }

    fn init_duplicator(&mut self) -> Result<(), ScreenCaptureError> {
        if self.output.is_none() {
            return Err(ScreenCaptureError::LogicError {
                msg: "cannot init duplicator without valid output".to_owned(),
            });
        }
        let output = self.output.as_ref().unwrap();
        self.duplicator = None;

        unsafe {
            let output1: Result<IDXGIOutput1, WinError> = output.cast();
            let output1 = output1.expect_with("output should be convertible to IDXGIOutput1");
            // let output1 = output.GetParent::<&IDXGIOutput1>().expect("Yes");
            // From C++, the following can fail with:
            //  E_ACCESSDENIED, when on fullscreen uac prompt
            //  DXGI_ERROR_SESSION_DISCONNECTED, somehow.
            if self.device.is_none() {
                return Err(ScreenCaptureError::LogicError {
                    msg: "device is none, is the adaptor initialised?".to_owned(),
                });
            }
            let duplicator = output1
                .DuplicateOutput(self.device.as_ref().unwrap())
                .map_err(logic_error)?;
            self.duplicator = Some(duplicator);

            let duplicator = self.duplicator.as_ref().unwrap();
            let mut desc: DXGI_OUTDUPL_DESC = DXGI_OUTDUPL_DESC {
                ModeDesc: DXGI_MODE_DESC {
                    Width: 0,
                    Height: 0,
                    RefreshRate: DXGI_RATIONAL {
                        Numerator: 0,
                        Denominator: 0,
                    },
                    Format: 0,
                    ScanlineOrdering: 0,
                    Scaling: 0,
                },
                Rotation: 0,
                DesktopImageInSystemMemory: windows::Win32::Foundation::BOOL(0),
            };
            duplicator.GetDesc(&mut desc);
            /*
            println!(
                "Duplicator initialised: {}x{} @ {}/{}, in memory: {}",
                desc.ModeDesc.Width,
                desc.ModeDesc.Height,
                desc.ModeDesc.RefreshRate.Numerator,
                desc.ModeDesc.RefreshRate.Denominator,
                desc.DesktopImageInSystemMemory.0
            );*/
        }
        Ok(())
    }

    pub fn new() -> Result<CaptureWin, ScreenCaptureError> {
        let mut n: CaptureWin = Default::default();
        n.init_adaptor()?;
        n.init_debug()?;
        Ok(n)
    }

    pub fn prepare(
        &mut self,
        display: u32,
        _x: u32,
        _y: u32,
        _width: u32,
        _height: u32,
    ) -> Result<(), ScreenCaptureError> {
        self.init_output(display)?;
        self.init_duplicator()?;
        Ok(())
    }

    pub fn capture(&mut self) -> Result<(), ScreenCaptureError> {
        // Ok, so, check if we have a duplicator.
        if self.duplicator.is_none() {
            return Err(ScreenCaptureError::LogicError {
                msg: format!("no duplicator to capture image, call prepare capture "),
            });
        }
        let duplicator = self.duplicator.as_ref().unwrap();

        // Now, we can acquire the next frame.
        let timeout_in_ms: u32 = 100;
        let mut frame_info: windows::Win32::Graphics::Dxgi::DXGI_OUTDUPL_FRAME_INFO =
            Default::default();
        let mut pp_desktop_resource: Option<IDXGIResource> = None;
        let res = unsafe {
            duplicator.AcquireNextFrame(timeout_in_ms, &mut frame_info, &mut pp_desktop_resource)
        };

        let release_frame = || {
            unsafe {
                // Ignore the frame release status.
                if let Some(duplicator) = self.duplicator.as_ref() {
                    let _ = duplicator.ReleaseFrame();
                }
            }
        };
        if let Err(ref r) = res {
            self.dump_debug_messages();
            // println!("got an error error!: {:?}", r);
            // Error handling from the c++ implementation.
            if r.code() == windows::Win32::Graphics::Dxgi::DXGI_ERROR_ACCESS_LOST {
                // This can happen when the resolution changes, or when we the context changes / full screen application
                // or a d3d11 instance starts, in that case we have to recreate the duplicator.
                release_frame();
                // In this situation, the application must release the IDXGIOutputDuplication interface and
                // create a new IDXGIOutputDuplication for the new content.
                // self.duplicator = None;
                // The other side will just re-initialise.
                return Err(ScreenCaptureError::LostCaptureError {
                    msg: format!("{r:?}"),
                });
            } else if r.code() == windows::Win32::Graphics::Dxgi::DXGI_ERROR_INVALID_CALL {
                // Dxgi invallid call, we can check the error messages here.

                // Well, we timed out, and we don't have any image... bummer.
                return Err(ScreenCaptureError::LostCaptureError {
                    msg: format!("{r:?}"),
                });
            } else if r.code() == windows::Win32::Graphics::Dxgi::DXGI_ERROR_WAIT_TIMEOUT {
                // Timeout may happen if no changes occured from the last frame.
                // This means it is perfectly ok to return the current image.
                if self.image.is_some() {
                    return Ok(()); // likely no draw events since last frame, return ok since we have a frame to show.
                }
                // Well, we timed out, and we don't have any image... bummer.
                return Err(ScreenCaptureError::TransientError {
                    msg: format!("{r:?}"),
                });
            } else if r.code() == windows::Win32::Foundation::D2DERR_INVALID_CALL {
                release_frame();
                // Some object went invalid, return an lost capture such that we re initialise.
                return Err(ScreenCaptureError::LostCaptureError {
                    msg: format!("{r:?}"),
                });
            } else {
                println!("Unhandled error!: {:?}", r);
                release_frame();
                return Err(ScreenCaptureError::LogicError {
                    msg: format!("{r:?}"),
                });
            }
        }

        // Well, we got here, res must be ok.
        let _ok = res.expect_with("Should be ok.");

        // Now, we can do something with textures and all that.
        let texture: Result<ID3D11Texture2D, WinError> = pp_desktop_resource
            .as_ref()
            .expect_with("Should be resource")
            .cast();
        let frame = texture.expect_with("Must be a texture.");
        let mut tex_desc: windows::Win32::Graphics::Direct3D11::D3D11_TEXTURE2D_DESC =
            Default::default();
        unsafe { frame.GetDesc(&mut tex_desc) };

        let mut img_desc: windows::Win32::Graphics::Direct3D11::D3D11_TEXTURE2D_DESC =
            Default::default();
        if let Some(img) = &self.image {
            unsafe { img.GetDesc(&mut img_desc) };
        }

        // Here, we create an texture that will be mapped.
        if self.image.is_none()
            || img_desc.Width != tex_desc.Width
            || img_desc.Height != tex_desc.Height
        {
            // No mapped image to use yet, or size is different. Create a new image using the device.
            let mut new_img: windows::Win32::Graphics::Direct3D11::D3D11_TEXTURE2D_DESC =
                Default::default();
            new_img.Width = tex_desc.Width;
            new_img.Height = tex_desc.Height;
            new_img.Format = tex_desc.Format;
            new_img.MipLevels = 1; // from C++ side.
            new_img.ArraySize = 1; // from C++ side.
            new_img.SampleDesc.Count = 1; // from C++ side.
            new_img.Usage = windows::Win32::Graphics::Direct3D11::D3D11_USAGE_STAGING;
            new_img.CPUAccessFlags = windows::Win32::Graphics::Direct3D11::D3D11_CPU_ACCESS_READ;

            self.image = Some(unsafe {
                self.device
                    .as_ref()
                    .expect_with("Must have device")
                    .CreateTexture2D(
                        &new_img,
                        0 as *const windows::Win32::Graphics::Direct3D11::D3D11_SUBRESOURCE_DATA,
                    )
                    .map_err(lost_capture_error)?
            });
        }

        // Finally, we are at the end of all of this and we can actually copy the resource.
        unsafe {
            self.device_context
                .as_ref()
                .expect_with("Should have a device context.")
                .CopyResource(self.image.as_ref().unwrap(), frame);
            let _ = self.duplicator.as_ref().unwrap().ReleaseFrame();
        }
        Ok(())
    }

    fn image(&mut self) -> Result<ImageWin, ScreenCaptureError> {
        // Need to make a new image here now, because we can't copy into mapped images, so we need to ensure we hand off a
        // fresh image.
        if self.image.is_none() {
            return Err(ScreenCaptureError::LogicError {
                msg: "capture needs to succeed before image retrieval".to_owned(),
            });
        }
        let image = self.image.as_ref().unwrap();

        let mut tex_desc: windows::Win32::Graphics::Direct3D11::D3D11_TEXTURE2D_DESC =
            Default::default();
        unsafe {
            image.GetDesc(&mut tex_desc);
        }

        let mut new_img: windows::Win32::Graphics::Direct3D11::D3D11_TEXTURE2D_DESC =
            Default::default();
        new_img.Width = tex_desc.Width;
        new_img.Height = tex_desc.Height;
        new_img.Format = tex_desc.Format;
        new_img.MipLevels = 1; // from C++ side.
        new_img.ArraySize = 1; // from C++ side.
        new_img.SampleDesc.Count = 1; // from C++ side.
        new_img.Usage = windows::Win32::Graphics::Direct3D11::D3D11_USAGE_STAGING;
        new_img.CPUAccessFlags = windows::Win32::Graphics::Direct3D11::D3D11_CPU_ACCESS_READ;
        let device = self.device.as_ref().expect_with("Must have a device");
        let new_texture = unsafe {
            // Need to wrap this into a releasing thing.
            device
                .CreateTexture2D(
                    &new_img,
                    0 as *const windows::Win32::Graphics::Direct3D11::D3D11_SUBRESOURCE_DATA,
                )
                .map_err(lost_capture_error)?
        };
        unsafe {
            self.device_context
                .as_ref()
                .expect_with("Should have a device context.")
                .CopyResource(&new_texture, image);
        }

        Ok(ImageWin::new(new_texture))
    }
}

impl Capture for CaptureWin {
    fn capture_image(&mut self) -> Result<(), ScreenCaptureError> {
        let res = CaptureWin::capture(self);

        res.map_err(|v| Into::<ScreenCaptureError>::into(v))
    }
    fn image(&mut self) -> std::result::Result<Box<dyn ImageBGR>, ScreenCaptureError> {
        Ok(Box::<ImageWin>::new(
            CaptureWin::image(self).map_err(|v| Into::<ScreenCaptureError>::into(v))?,
        ))
    }

    fn resolution(&mut self) -> Resolution {
        Resolution {
            width: 0,
            height: 0,
        }
    }

    fn prepare_capture(
        &mut self,
        display: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<(), ScreenCaptureError> {
        CaptureWin::prepare(self, display, x, y, width, height)
    }
}

pub fn capture() -> Result<Box<dyn Capture>, ScreenCaptureError> {
    let capture_win = CaptureWin::new()?;
    let z = Box::<CaptureWin>::new(capture_win);
    Ok(z)
}
