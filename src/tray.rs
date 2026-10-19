use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use std::sync::mpsc::Sender;
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
#[cfg(windows)]
use tao::platform::windows::EventLoopBuilderExtWindows;
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub enum TrayCommand {
    Quit,
    ShowLine,
    ToggleAlwaysVisible,
    Clear,
    RestoreDismissed,
    RequestMountFolder,
}

enum UserEvent {
    Menu(MenuEvent),
}

fn tray_icon() -> Icon {
    let mut rgba = vec![0u8; 16 * 16 * 4];
    for y in 0..16 {
        for x in 0..16 {
            let i = (y * 16 + x) * 4;
            // 简易晾衣绳图标：顶栏 + 彩色夹
            if y <= 1 {
                rgba[i] = 120;
                rgba[i + 1] = 90;
                rgba[i + 2] = 60;
                rgba[i + 3] = 255;
            } else if y >= 4 && y <= 7 && (x == 3 || x == 7 || x == 11) {
                let c = [(220, 60, 60), (240, 160, 40), (60, 160, 220)][x / 4];
                rgba[i] = c.0;
                rgba[i + 1] = c.1;
                rgba[i + 2] = c.2;
                rgba[i + 3] = 255;
            } else {
                rgba[i + 3] = 0;
            }
        }
    }
    Icon::from_rgba(rgba, 16, 16).expect("icon")
}

fn build_menu(zh: bool, always: bool) -> Menu {
    let menu = Menu::new();
    let vis = if always {
        if zh {
            "一直显示 ✓"
        } else {
            "Always visible ✓"
        }
    } else if zh {
        "自动隐藏"
    } else {
        "Auto hide"
    };
    let _ = menu.append(&MenuItem::with_id(
        "show",
        if zh { "展开晾衣绳" } else { "Show clothesline" },
        true,
        None,
    ));
    let _ = menu.append(&MenuItem::with_id("toggle_vis", vis, true, None));
    let _ = menu.append(&MenuItem::with_id(
        "mount",
        if zh { "挂上文件夹…" } else { "Mount folder…" },
        true,
        None,
    ));
    let _ = menu.append(&MenuItem::with_id(
        "restore",
        if zh { "重新显示已取下" } else { "Restore dismissed" },
        true,
        None,
    ));
    let _ = menu.append(&MenuItem::with_id(
        "clear",
        if zh { "清空" } else { "Clear" },
        true,
        None,
    ));
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&MenuItem::with_id(
        "quit",
        if zh { "退出" } else { "Quit" },
        true,
        None,
    ));
    menu
}

pub fn run_tray_thread(cmd_tx: Sender<TrayCommand>, locale_zh: bool, always_visible: bool) {
    std::thread::spawn(move || {
        let mut builder = EventLoopBuilder::<UserEvent>::with_user_event();
        #[cfg(windows)]
        builder.with_any_thread(true);
        let event_loop = builder.build();
        let proxy = event_loop.create_proxy();
        MenuEvent::set_event_handler(Some(move |event| {
            let _ = proxy.send_event(UserEvent::Menu(event));
        }));

        let mut zh = locale_zh;
        let mut always = always_visible;
        let tray: std::cell::RefCell<Option<TrayIcon>> = std::cell::RefCell::new(None);

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;
            match event {
                Event::NewEvents(StartCause::Init) => {
                    let icon = TrayIconBuilder::new()
                        .with_menu(Box::new(build_menu(zh, always)))
                        .with_tooltip(if zh {
                            "晾衣绳 Clothesline"
                        } else {
                            "Clothesline"
                        })
                        .with_icon(tray_icon())
                        .build()
                        .expect("tray");
                    *tray.borrow_mut() = Some(icon);
                }
                Event::UserEvent(UserEvent::Menu(ev)) => match ev.id.as_ref() {
                    "show" => {
                        let _ = cmd_tx.send(TrayCommand::ShowLine);
                    }
                    "toggle_vis" => {
                        always = !always;
                        if let Some(t) = tray.borrow().as_ref() {
                            let _ = t.set_menu(Some(Box::new(build_menu(zh, always))));
                        }
                        let _ = cmd_tx.send(TrayCommand::ToggleAlwaysVisible);
                    }
                    "mount" => {
                        let _ = cmd_tx.send(TrayCommand::RequestMountFolder);
                    }
                    "restore" => {
                        let _ = cmd_tx.send(TrayCommand::RestoreDismissed);
                    }
                    "clear" => {
                        let _ = cmd_tx.send(TrayCommand::Clear);
                    }
                    "quit" => {
                        let _ = cmd_tx.send(TrayCommand::Quit);
                        *control_flow = ControlFlow::Exit;
                    }
                    _ => {}
                },
                _ => {}
            }
        });
    });
}
