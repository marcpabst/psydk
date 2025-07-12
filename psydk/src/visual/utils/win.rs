use std::thread;
use std::time::{Duration, Instant};
use windows::Wdk::Graphics::Direct3D::{
    D3DKMTEnumAdapters3, D3DKMTGetScanLine, D3DKMT_ENUMADAPTERS3, D3DKMT_GETSCANLINE,
};

use windows::Win32::Devices::Display::DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::{
    core::Result as Win32Result,
    Win32::System::Performance::QueryPerformanceFrequency,
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::Threading::{
            CancelWaitableTimer, CreateWaitableTimerExW, SetWaitableTimer, WaitForSingleObject,
            CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CREATE_WAITABLE_TIMER_MANUAL_RESET, TIMER_ALL_ACCESS,
        },
    },
};

use windows::{
    core::Error,
    Win32::Devices::Display::{
        DisplayConfigGetDeviceInfo, QueryDisplayConfig, DISPLAYCONFIG_DEVICE_INFO_HEADER,
        DISPLAYCONFIG_DEVICE_INFO_TYPE, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
        DISPLAYCONFIG_SOURCE_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
    },
    Win32::Graphics::{
        Dxgi::{IDXGIAdapter, IDXGIOutput, IDXGISwapChain, DXGI_ADAPTER_DESC, DXGI_OUTPUT_DESC},
        Gdi::{GetMonitorInfoW, MONITORINFOEXW},
    },
};

/// A high-precision timer implementation for Windows using the Win32 API.
struct HighPrecisionTimer {
    timer: HANDLE,
    period_microseconds: u32,
}

impl HighPrecisionTimer {
    fn new(period_microseconds: u32) -> Win32Result<Self> {
        unsafe {
            let timer = CreateWaitableTimerExW(
                None,
                None,
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION | CREATE_WAITABLE_TIMER_MANUAL_RESET,
                TIMER_ALL_ACCESS.0,
            )?;

            let due_time = (period_microseconds * 10) as i64; // 100-nanosecond intervals

            SetWaitableTimer(timer, &-due_time, 0, None, None, false)?;

            Ok(Self {
                timer,
                period_microseconds,
            })
        }
    }

    fn wait(&self) -> Win32Result<()> {
        unsafe {
            WaitForSingleObject(self.timer, u32::MAX);
            // Reset the timer for the next wait
            let due_time = (self.period_microseconds * 10) as i64; // 100-nanosecond intervals
            SetWaitableTimer(self.timer, &-due_time, 0, None, None, false)?;
        }
        Ok(())
    }

    fn cancel(&self) -> Win32Result<()> {
        unsafe { CancelWaitableTimer(self.timer) }
    }
}

pub fn get_adapter_and_vidpn_source(swap_chain: &IDXGISwapChain) {
    //-> Win32Result<(HANDLE, u32)> {
    // 1. Get the output (monitor) the swap chain is presenting to
    let output: IDXGIOutput =
        unsafe { swap_chain.GetContainingOutput() }.expect("Failed to get output from swap chain");

    // 2. Get the output's HMONITOR and device name
    let output_desc: DXGI_OUTPUT_DESC = unsafe { output.GetDesc() }.expect("Failed to get output description");
    let hmonitor: HMONITOR = output_desc.Monitor;

    // 3. Use EnumDisplayMonitors to list all monitors and find the one matching our HMONITOR
    let mut monitor_info = MONITORINFOEXW::default();
    monitor_info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

    unsafe {
        if !GetMonitorInfoW(hmonitor, &mut monitor_info as *mut _ as _).as_bool() {
            panic!("Failed to get monitor info");
        }
    }

    let device_name = String::from_utf16_lossy(
        &monitor_info
            .szDevice
            .iter()
            .take_while(|&&c| c != 0)
            .cloned()
            .collect::<Vec<_>>(),
    );

    println!("Monitor device name: {}", device_name);

    // 3. Use QueryDisplayConfig to find the VidPnSourceId for this device
    let mut num_paths = 128u32;
    let mut num_modes = 128u32;

    let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); num_paths as usize];
    let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); num_modes as usize];
    unsafe {
        QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut num_paths,
            paths.as_mut_ptr(),
            &mut num_modes,
            modes.as_mut_ptr(),
            None,
        );
    }

    println!(
        "QueryDisplayConfig returned {} paths and {} modes",
        num_paths, num_modes
    );

    // truncate the vectors to the actual number of paths and modes
    paths.truncate(num_paths as usize);
    modes.truncate(num_modes as usize);

    // Find the path whose source device name matches our monitor
    let mut vidpn_source_id = None;
    for path in &paths {
        let source_id = path.sourceInfo.id;
        let adapter_id = path.sourceInfo.adapterId;

        let mut source_name = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                size: std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                adapterId: adapter_id,
                id: source_id,
            },
            viewGdiDeviceName: [0; 32],
        };

        let status = unsafe { DisplayConfigGetDeviceInfo(&mut source_name as *mut _ as _) };
        println!("DisplayConfigGetDeviceInfo status: {}", status);
        if status == 0 {
            let dev_name = String::from_utf16_lossy(
                &source_name
                    .viewGdiDeviceName
                    .iter()
                    .take_while(|&&c| c != 0)
                    .cloned()
                    .collect::<Vec<_>>(),
            );
            if dev_name == device_name {
                vidpn_source_id = Some(source_id);
                break;
            }
        }
    }
    let vidpn_source_id = vidpn_source_id.expect("Failed to find VidPnSourceId for the output");

    println!("Found VidPnSourceId: {}", vidpn_source_id);

    // // 4. Get the adapter LUID from the swap chain's device
    // let adapter: IDXGIAdapter = unsafe {
    //     let mut adapter = None;
    //     swap_chain.GetDevice(&IDXGIAdapter::IID, &mut adapter as *mut _ as _)?;
    //     adapter.ok_or_else(|| Error::from_win32())?
    // };
    // let desc: DXGI_ADAPTER_DESC = unsafe {
    //     let mut desc = std::mem::zeroed();
    //     adapter.GetDesc(&mut desc)?;
    //     desc
    // };

    // // 5. Open the adapter using D3DKMTOpenAdapterFromLuid
    // let mut open_adapter = D3DKMT_OPENADAPTERFROMLUID {
    //     AdapterLuid: desc.AdapterLuid,
    //     hAdapter: 0,
    // };
    // let status = unsafe { D3DKMTOpenAdapterFromLuid(&mut open_adapter) };
    // if status != 0 {
    //     return Err(Error::from_win32());
    // }
    // let h_adapter = open_adapter.hAdapter as HANDLE;

    // Ok((h_adapter, vidpn_source_id))
}

pub fn get_qpc_frequency() -> i64 {
    let mut frequency: i64 = 0;
    unsafe {
        QueryPerformanceFrequency(&mut frequency);
    }
    frequency
}
