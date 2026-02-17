#[cfg(target_os = "ios")]
pub mod face_tracking {

    use numpy::{IntoPyArray, PyArray2, PyReadonlyArray2, ToPyArray};
    use objc2::runtime::ProtocolObject;
    use objc2::AnyThread;
    use objc2::DefinedClass;
    use objc2::RefEncode;
    use std::sync::Arc;

    use pyo3::prelude::*;

    use std::arch::aarch64::float32x4_t;
    use std::cell::RefCell;
    use std::time::SystemTime;
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
        delegate: Retained<Delegate>,
    }

    #[pyclass]
    #[derive(Clone)]
    pub struct Face {
        pub camera_transform: nalgebra::Matrix4<f32>,
        pub left_eye_transform: nalgebra::Matrix4<f32>,
        pub right_eye_transform: nalgebra::Matrix4<f32>,
        pub face_transform: nalgebra::Matrix4<f32>,
    }

    #[pyclass]
    #[derive(Clone)]
    pub struct FaceTrackingFrame {
        pub timestamp: SystemTime,
        pub faces: Vec<Face>,
    }

    // SAFETY: ARSession is not thread safe by default, but we ensure that all access to it is synchronized via a Mutex in FaceTracker
    unsafe impl Send for FaceTracker {}
    unsafe impl Sync for FaceTracker {}

    #[derive(Clone)]
    struct FaceTrackingQueue {
        inner: Arc<Mutex<Vec<FaceTrackingFrame>>>,
        max_size: usize,
    }

    impl FaceTrackingQueue {
        pub fn new() -> Self {
            FaceTrackingQueue {
                inner: Arc::new(Mutex::new(Vec::new())),
                max_size: 10000,
            }
        }

        pub fn with_capacity(capacity: usize) -> Self {
            FaceTrackingQueue {
                inner: Arc::new(Mutex::new(Vec::with_capacity(capacity))),
                max_size: capacity,
            }
        }

        pub fn push(&self, result: FaceTrackingFrame) {
            let mut vec = self.inner.lock().unwrap();
            vec.push(result);
            // limit size to 10
            if vec.len() > self.max_size {
                vec.remove(0);
            }
        }

        pub fn clear(&self) {
            self.inner.lock().unwrap().clear();
        }

        pub fn len(&self) -> usize {
            self.inner.lock().unwrap().len()
        }

        pub fn last(&self) -> Option<FaceTrackingFrame> {
            self.inner.lock().unwrap().last().cloned()
        }

        pub fn all(&self) -> Vec<FaceTrackingFrame> {
            self.inner.lock().unwrap().clone()
        }

        pub fn drain(&self) -> Vec<FaceTrackingFrame> {
            let mut vec = self.inner.lock().unwrap();
            std::mem::take(&mut *vec)
        }
    }

    #[derive(Clone)]
    struct DelegateIvars {
        queue: FaceTrackingQueue,
    }

    // implement Encode for FaceTrackingFrame
    unsafe impl Encode for FaceTrackingQueue {
        const ENCODING: Encoding = Encoding::Object;
    }

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

    // ARSessionDelegate implementation
    define_class!(
        #[unsafe(super = NSObject)]
        #[ivars = DelegateIvars]
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
            fn session_did_update_frame(&self, session: &ARSession, frame: &ARFrame) {
                // convert to FaceTrackingFrame and store in queue
                let anchors = unsafe { frame.anchors() };
                let mut results = FaceTrackingFrame {
                    timestamp: SystemTime::now(),
                    faces: Vec::new(),
                };
                for anchor in anchors.iter() {
                    // check if anchor is ARFaceAnchor and if activly being tracked
                    if anchor.isKindOfClass(&objc2_ar_kit::ARFaceAnchor::class()) {
                        // extract face position
                        let face_anchor: &objc2_ar_kit::ARFaceAnchor = anchor.downcast_ref().unwrap();

                        if !unsafe { face_anchor.isTracked() } {
                            continue;
                        }

                        let left_eye_transform = unsafe { get_transform(face_anchor, sel!(leftEyeTransform)) };
                        let right_eye_transform = unsafe { get_transform(face_anchor, sel!(rightEyeTransform)) };
                        let face_transform = unsafe { get_transform(face_anchor, sel!(transform)) };

                        // get the camera transform
                        let camera = unsafe { frame.camera() };
                        let camera_transform = unsafe { get_transform(&camera, sel!(transform)) };

                        results.faces.push(Face {
                            camera_transform,
                            left_eye_transform,
                            right_eye_transform,
                            face_transform,
                        });
                    }
                }
                // store results in ivars
                self.ivars().queue.push(results);
            }
        }
    );

    // Add creation method.
    impl Delegate {
        fn new() -> Retained<Self> {
            // Initialize instance variables.
            let this = Self::alloc().set_ivars(DelegateIvars {
                queue: FaceTrackingQueue::new(),
            });
            // Call `NSObject`'s `init` method.
            unsafe { msg_send![super(this), init] }
        }
    }

    impl Delegate {
        extern_methods!(
            #[unsafe(method(queue))]
            pub fn get_queue(&self) -> FaceTrackingQueue;
        );
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
            let delegate = Delegate::new();
            unsafe {
                session.runWithConfiguration(&configuration);
                session.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            }
            FaceTracker {
                ar_session: Mutex::new(session),
                delegate,
            }
        }

        pub fn last_frame(&self) -> Option<FaceTrackingFrame> {
            self.delegate.ivars().queue.last()
        }

        pub fn all(&self) -> Vec<FaceTrackingFrame> {
            self.delegate.ivars().queue.all()
        }

        pub fn drain(&self) -> Vec<FaceTrackingFrame> {
            self.delegate.ivars().queue.drain()
        }
    }

    #[pymethods]
    impl FaceTrackingFrame {
        pub fn timestamp(&self) -> Option<f64> {
            match self.timestamp.duration_since(SystemTime::UNIX_EPOCH) {
                Ok(dur) => Some(dur.as_secs_f64()),
                Err(_) => None,
            }
        }

        pub fn faces(&self) -> Vec<Face> {
            self.faces.clone()
        }
    }

    #[pymethods]
    impl Face {
        pub fn mean_eye_distance(&self) -> Option<f32> {
            // get the camera transform

            let camera_position = self.camera_transform.row(3).transpose().xyz();

            // eye position needs to be multiplied by face transform
            // note thate everyting is a row-major 4x4 affine matrix
            let left_eye_position = (self.face_transform * self.left_eye_transform).row(3).transpose().xyz();
            let right_eye_position = (self.face_transform * self.right_eye_transform)
                .row(3)
                .transpose()
                .xyz();

            let camera_position = self.camera_transform.row(3).transpose().xyz();

            let left_eye_distance = (left_eye_position - camera_position).norm();
            let right_eye_distance = (right_eye_position - camera_position).norm();

            return Some((left_eye_distance + right_eye_distance) / 2.0);
        }

        pub fn left_eye_transform<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray2<f32>> {
            self.left_eye_transform.to_pyarray(py)
        }

        pub fn right_eye_transform<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray2<f32>> {
            self.right_eye_transform.to_pyarray(py)
        }

        pub fn face_transform<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray2<f32>> {
            self.face_transform.to_pyarray(py)
        }

        pub fn camera_transform<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray2<f32>> {
            self.camera_transform.to_pyarray(py)
        }
    }
}
