use super::prelude::*;

static DEBUG_LAYER: Mutex<String> = Mutex::new(String::new());
pub(crate) fn enable_debug_layer() {
    unsafe {
        let mut debug: Option<ID3D12Debug> = None;
        let status = match D3D12GetDebugInterface(&mut debug) {
            Ok(()) => match debug {
                Some(debug) => {
                    debug.EnableDebugLayer();
                    "enabled".to_string()
                }
                None => "GetDebugInterface returned no object".to_string(),
            },
            Err(err) => format!("GetDebugInterface: {err}"),
        };
        if let Ok(mut slot) = DEBUG_LAYER.lock() {
            *slot = status;
        }
    }
}
pub(crate) fn clear_dxgi_messages() {
    let Ok(queue) = (unsafe { DXGIGetDebugInterface1::<IDXGIInfoQueue>(0) }) else {
        return;
    };
    unsafe { queue.ClearStoredMessages(DXGI_DEBUG_ALL) };
}
pub(crate) fn dxgi_debug_messages() -> String {
    let queue = match unsafe { DXGIGetDebugInterface1::<IDXGIInfoQueue>(0) } {
        Ok(queue) => queue,
        Err(err) => return format!("dxgi debug unavailable: {err}"),
    };
    let count = unsafe { queue.GetNumStoredMessages(DXGI_DEBUG_ALL) };
    if count == 0 {
        return "dxgi debug stored no messages".to_string();
    }
    let mut lines = Vec::new();
    let start = count.saturating_sub(6);
    for index in start..count {
        let mut length = 0usize;
        if unsafe { queue.GetMessage(DXGI_DEBUG_ALL, index, None, &mut length) }.is_err()
            || length == 0
        {
            continue;
        }
        let mut bytes = vec![0u8; length];
        let message = bytes.as_mut_ptr().cast::<DXGI_INFO_QUEUE_MESSAGE>();
        if unsafe { queue.GetMessage(DXGI_DEBUG_ALL, index, Some(message), &mut length) }.is_err() {
            continue;
        }
        let message = unsafe { &*message };
        let description = if message.pDescription.is_null() || message.DescriptionByteLength == 0 {
            String::new()
        } else {
            let raw = unsafe {
                std::slice::from_raw_parts(message.pDescription, message.DescriptionByteLength)
            };
            String::from_utf8_lossy(raw)
                .trim_end_matches('\0')
                .to_string()
        };
        lines.push(description);
    }
    if lines.is_empty() {
        "dxgi debug stored no readable messages".to_string()
    } else {
        lines.join(" || ")
    }
}
pub(crate) fn clear_debug_messages(device: &ID3D12Device) {
    let Ok(info) = device.cast::<ID3D12InfoQueue>() else {
        return;
    };
    unsafe { info.ClearStoredMessages() };
}
pub(crate) fn debug_messages(device: &ID3D12Device) -> String {
    let status = DEBUG_LAYER
        .lock()
        .map(|slot| slot.clone())
        .unwrap_or_else(|_| "debug status unavailable".to_string());
    let Ok(info) = device.cast::<ID3D12InfoQueue>() else {
        return format!("debug layer unavailable ({status})");
    };
    let count = unsafe { info.GetNumStoredMessages() };
    if count == 0 {
        return "debug layer stored no messages".to_string();
    }
    let mut lines = Vec::new();
    for index in 0..count {
        let mut length = 0usize;
        if unsafe { info.GetMessage(index, None, &mut length) }.is_err() || length == 0 {
            continue;
        }
        let mut bytes = vec![0u8; length];
        let message = bytes.as_mut_ptr().cast::<D3D12_MESSAGE>();
        if unsafe { info.GetMessage(index, Some(message), &mut length) }.is_err() {
            continue;
        }
        let message = unsafe { &*message };
        let description = if message.pDescription.is_null() || message.DescriptionByteLength == 0 {
            String::new()
        } else {
            let raw = unsafe {
                std::slice::from_raw_parts(message.pDescription, message.DescriptionByteLength)
            };
            String::from_utf8_lossy(raw)
                .trim_end_matches('\0')
                .to_string()
        };
        lines.push(format!("{:?} {:?}", message.Severity, description));
    }
    if lines.is_empty() {
        "debug layer stored no readable messages".to_string()
    } else {
        lines.join(" || ")
    }
}
