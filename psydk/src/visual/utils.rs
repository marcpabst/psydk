// // function that returns a u64 frame number of the last frame submitted to a wgpu::Surface
// fn get_last_frame_number(surface: &wgpu::Surface) -> u64 {
//     // on DX12, the frame number can be retrieved from the swap chain
//     #[cfg(feature = "dx12")]
//     let frame_id =
//         unsafe { surface.as_hal::<wgpu::hal::api::Metal, _, _>(|surface| surface.swap_chain().GetLastPresentCount()) };
//     // on macos, the frame number can be retrieved from the queue
//     #[cfg(feature = "metal")]
//     let frame_id =
//         unsafe { surface.as_hal::<wgpu::hal::api::Metal, _, _>(|surface| surface.queue().get_last_frame_id()) };
//     frame_id as u64
// }
