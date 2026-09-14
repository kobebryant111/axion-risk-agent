//! Windows：启动前检测 Edge WebView2；缺失则下载官方引导程序并安装。
//! macOS 使用系统 WKWebView，无需处理。

pub fn ensure_or_install() {
    #[cfg(windows)]
    windows_impl::ensure_or_install();
}

#[cfg(windows)]
mod windows_impl {
    use std::fs;
    use std::io::Write;
    use std::os::windows::process::CommandExt;
    use std::path::PathBuf;
    use std::process::Command;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDYES, MB_ICONERROR, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MB_YESNO,
    };
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    const CLIENT_GUID: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    const BOOTSTRAPPER_URL: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    pub fn ensure_or_install() {
        if webview2_installed() {
            return;
        }

        let yes = message(
            "智鉴风控官需要 Microsoft Edge WebView2 才能显示界面。\n\n未检测到运行时，是否现在自动下载并安装？\n（安装过程可能弹出系统权限确认）",
            "需要安装 WebView2",
            MB_YESNO | MB_ICONWARNING,
        );
        if yes != IDYES {
            message(
                "已取消。请安装 WebView2 后再打开本程序：\nhttps://go.microsoft.com/fwlink/p/?LinkId=2124703",
                "无法启动",
                MB_OK | MB_ICONINFORMATION,
            );
            std::process::exit(0);
        }

        match download_and_run_bootstrapper() {
            Ok(()) if webview2_installed() => {
                message(
                    "WebView2 已安装完成，即将启动智鉴风控官。",
                    "安装成功",
                    MB_OK | MB_ICONINFORMATION,
                );
            }
            Ok(()) => {
                message(
                    "安装程序已结束，但仍未检测到 WebView2。\n请重启电脑后再试，或手动打开：\nhttps://go.microsoft.com/fwlink/p/?LinkId=2124703",
                    "仍未就绪",
                    MB_OK | MB_ICONERROR,
                );
                std::process::exit(1);
            }
            Err(e) => {
                message(
                    &format!(
                        "自动下载/安装失败：{e}\n\n请用浏览器打开下面地址手动安装后重试：\n{BOOTSTRAPPER_URL}"
                    ),
                    "安装失败",
                    MB_OK | MB_ICONERROR,
                );
                let _ = Command::new("cmd")
                    .args(["/C", "start", "", BOOTSTRAPPER_URL])
                    .creation_flags(CREATE_NO_WINDOW)
                    .status();
                std::process::exit(1);
            }
        }
    }

    fn webview2_installed() -> bool {
        let paths = [
            format!(r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{CLIENT_GUID}"),
            format!(r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{CLIENT_GUID}"),
        ];
        for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
            let hk = RegKey::predef(root);
            for path in &paths {
                if let Ok(key) = hk.open_subkey_with_flags(path, KEY_READ) {
                    if let Ok(pv) = key.get_value::<String, _>("pv") {
                        let pv = pv.trim();
                        if !pv.is_empty() && pv != "0.0.0.0" {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn download_and_run_bootstrapper() -> Result<(), String> {
        message(
            "即将从 Microsoft 下载 WebView2 安装程序，请保持网络畅通。",
            "正在准备下载",
            MB_OK | MB_ICONINFORMATION,
        );

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .redirect(reqwest::redirect::Policy::limited(10))
            .user_agent("AxiomRiskAgent/0.1 (WebView2 bootstrapper)")
            .build()
            .map_err(|e| e.to_string())?;

        let resp = client
            .get(BOOTSTRAPPER_URL)
            .send()
            .map_err(|e| format!("网络错误: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("下载失败 HTTP {}", resp.status()));
        }
        let bytes = resp.bytes().map_err(|e| e.to_string())?;
        if bytes.len() < 1024 {
            return Err("下载内容过短，可能被网络拦截".into());
        }

        let path = installer_path();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let mut file = fs::File::create(&path).map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        drop(file);

        let status = Command::new(&path)
            .status()
            .map_err(|e| format!("无法启动安装程序: {e}"))?;
        if !status.success() {
            return Err(format!("安装程序退出码 {:?}", status.code()));
        }
        Ok(())
    }

    fn installer_path() -> PathBuf {
        std::env::temp_dir().join("axiom-webview2-bootstrapper.exe")
    }

    fn message(text: &str, caption: &str, flags: u32) -> i32 {
        let text = wide(text);
        let caption = wide(caption);
        unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), flags) }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
}
