use pyo3::prelude::*;

#[cfg(target_os = "ios")]
mod face_tracking {

    use std::arch::aarch64::float32x4_t;
    use std::{arch::asm, ffi::c_void, mem::transmute, sync::Mutex};

    use objc2::{
        define_class, msg_send,
        rc::{Id, Retained},
        runtime::{AnyObject, Imp, NSObject, NSObjectProtocol, Sel},
        sel, ClassType,
    };

    use objc2::{extern_methods, Encode, Encoding};
    use objc2_foundation::{NSError, NSString, NSValue};

    use objc2_ar_kit::{
        ARFaceTrackingConfiguration, ARFrame, ARSession, ARSessionDelegate, ARSessionObserver, ARTrackable,
        ARWorldAlignment,
    };

    #[pyclass]
    pub struct FaceTracker {
        ar_session: Mutex<Retained<ARSession>>,
    }

    // SAFETY: ARSession is not thread safe by default, but we ensure that all access to it is synchronized via a Mutex in FaceTracker
    unsafe impl Send for FaceTracker {}
    unsafe impl Sync for FaceTracker {}

    // ARSessionDelegate implementation
    define_class!(
        #[unsafe(super = NSObject)]
        struct Delegate;

        unsafe impl NSObjectProtocol for Delegate {}

        unsafe impl ARSessionObserver for Delegate {
            #[unsafe(method(session:didFailWithError:))]
            fn session_did_fail_with_error(&self, _session: &ARSession, _error: &NSError) {
                // Handle session failure here
            }
        }

        unsafe impl ARSessionDelegate for Delegate {
            #[unsafe(method(session:didUpdateFrame:))]
            fn session_did_update_frame(&self, _session: &ARSession, _frame: &ARFrame) {
                // Handle frame updates here
            }
        }
    );

    pub unsafe fn get_transform(anchor: &AnyObject, selector: Sel) -> nalgebra::Matrix4<f32> {
        // 1. Get the Raw Pointer for the Selector
        // Since Sel is repr(transparent), we can transmute it to a raw void pointer
        // to satisfy the assembly input requirements.
        let sel_ptr: *const c_void = transmute(selector);

        // 2. Get the IMP
        let imp: unsafe extern "C" fn() = msg_send![anchor, methodForSelector: selector];

        // 3. Prepare outputs
        let c0: float32x4_t;
        let c1: float32x4_t;
        let c2: float32x4_t;
        let c3: float32x4_t;

        // 4. Assembly Call
        asm!(
            "blr {imp}",
            imp = in(reg) imp,
            in("x0") anchor as *const AnyObject, // Self
            in("x1") sel_ptr,                 // _cmd (Selector raw pointer)
            lateout("v0") c0,
            lateout("v1") c1,
            lateout("v2") c2,
            lateout("v3") c3,
            clobber_abi("C"),
        );

        // convert to float32x4_t to [f32; 4]
        let c0: [f32; 4] = transmute(c0);
        let c1: [f32; 4] = transmute(c1);
        let c2: [f32; 4] = transmute(c2);
        let c3: [f32; 4] = transmute(c3);

        nalgebra::Matrix4::from_rows(&[
            nalgebra::RowVector4::from(c0),
            nalgebra::RowVector4::from(c1),
            nalgebra::RowVector4::from(c2),
            nalgebra::RowVector4::from(c3),
        ])
    }

    /// IOS tracking functionality
    #[pymethods]
    impl FaceTracker {
        #[new]
        pub fn new() -> Self {
            let configuration = unsafe { ARFaceTrackingConfiguration::new() };
            unsafe {
                configuration.setWorldAlignment(ARWorldAlignment::Camera);
            }

            let session = unsafe { ARSession::new() };
            unsafe {
                session.runWithConfiguration(&configuration);
            }
            FaceTracker {
                ar_session: Mutex::new(session),
            }
        }

        pub fn get_last_face_distance(&self) -> Option<f32> {
            let session = self.ar_session.lock().unwrap();
            let current_frame = unsafe { session.currentFrame() };
            if let Some(frame) = current_frame {
                let anchors = unsafe { frame.anchors() };
                for anchor in anchors.iter() {
                    // check if anchor is ARFaceAnchor and if activly being tracked

                    if anchor.isKindOfClass(&objc2_ar_kit::ARFaceAnchor::class()) {
                        println!("Anchor: {:?}", anchor);
                        // extract face position
                        let face_anchor: &objc2_ar_kit::ARFaceAnchor = anchor.downcast_ref().unwrap();

                        if !unsafe { face_anchor.isTracked() } {
                            continue;
                        }

                        // let transform = unsafe { face_anchor.leftEyeTransform() };
                        // get leftEyeTransform use send_msg!
                        // let re_ransform: Simd4x4 = unsafe { msg_send!(face_anchor, rightEyeTransform) };
                        //     let selector = sel!(leftEyeTransform);
                        let left_eye_transform = unsafe { get_transform(face_anchor, sel!(leftEyeTransform)) };
                        let right_eye_transform = unsafe { get_transform(face_anchor, sel!(rightEyeTransform)) };
                        let face_transform = unsafe { get_transform(face_anchor, sel!(transform)) };

                        // get the camera transform
                        let camera = unsafe { frame.camera() };
                        let camera_transform = unsafe { get_transform(&camera, sel!(transform)) };

                        // eye position needs to be multiplied by face transform
                        // note thate everyting is a row-major 4x4 affine matrix
                        let left_eye_position = (face_transform * left_eye_transform).row(3).transpose().xyz();
                        let right_eye_position = (face_transform * right_eye_transform).row(3).transpose().xyz();

                        let camera_position = camera_transform.row(3).transpose().xyz();

                        let left_eye_distance = (left_eye_position - camera_position).norm();
                        let right_eye_distance = (right_eye_position - camera_position).norm();

                        return Some((left_eye_distance + right_eye_distance) / 2.0);
                    }
                }
            }
            None
        }
    }
}

/// A Python module implemented in Rust.
#[pymodule]
fn psydk_sensors(m: &Bound<'_, PyModule>) -> PyResult<()> {
    #[cfg(target_os = "ios")]
    m.add_class::<face_tracking::FaceTracker>()?;
    Ok(())
}
