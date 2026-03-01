use anyhow::Result;
use eframe::egui;
#[cfg(target_os = "macos")]
use muda::ContextMenu;
use muda::{Menu, MenuId, MenuItem};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tray_icon::{TrayIcon, TrayIconBuilder};

#[cfg(target_os = "macos")]
mod macos_status_bar;
#[cfg(target_os = "macos")]
use macos_status_bar::MonospaceStatusBar;

// タイマーの状態
#[derive(Clone, Copy, PartialEq, Debug)]
enum TimerState {
    Idle,
    Work(u64),         // 残り秒数
    WorkOvertime(u64), // 延長時間（秒）
    Break(u64),        // 残り秒数
}

impl TimerState {
    fn get_tray_text(&self, work_duration: u64) -> String {
        match self {
            TimerState::Idle => "Idle".to_string(),
            TimerState::Work(t) => format!("{:02}:{:02}", t / 60, t % 60),
            TimerState::WorkOvertime(overtime) => {
                let total = work_duration + overtime;
                format!("{:02}:{:02}", total / 60, total % 60)
            }
            TimerState::Break(t) => format!("{:02}:{:02}", t / 60, t % 60),
        }
    }

    fn get_icon_name(&self) -> &'static str {
        match self {
            TimerState::Idle => "stopTemplate.png",
            TimerState::Work(_) | TimerState::WorkOvertime(_) => "focusTemplate.png",
            TimerState::Break(_) => "breakTemplate.png",
        }
    }
}

// 共有状態（トレイアイコンとアプリ間で共有）
struct SharedState {
    state: TimerState,
    should_focus: bool,
}

fn main() -> Result<()> {
    // 共有状態
    let shared_state = Arc::new(Mutex::new(SharedState {
        state: TimerState::Idle,
        should_focus: false,
    }));

    // メニュー作成
    let show_app_id = MenuId::new("show_app");
    let quit_id = MenuId::new("quit");
    let menu = Menu::new();
    let show_app_item = MenuItem::with_id(
        show_app_id.clone(),
        "Show app",
        true,
        Some(muda::accelerator::Accelerator::new(
            None,
            muda::accelerator::Code::KeyO,
        )),
    );
    let quit_item = MenuItem::with_id(
        quit_id.clone(),
        "Quit",
        true,
        Some(muda::accelerator::Accelerator::new(
            None,
            muda::accelerator::Code::KeyQ,
        )),
    );
    menu.append(&show_app_item)?;
    menu.append(&quit_item)?;

    // macOSではmudaのメニューをMonospaceStatusBarに渡すため、tray-iconにはメニューを設定しない
    #[cfg(target_os = "macos")]
    let ns_menu_ptr = menu.ns_menu();

    #[cfg(target_os = "macos")]
    let _menu = menu; // mudaのMenuを保持してNSMenuの寿命を保証

    #[cfg(target_os = "macos")]
    let tray_icon = TrayIconBuilder::new()
        .with_tooltip("Focus Reactor")
        .with_title("")
        .build()?;

    #[cfg(not(target_os = "macos"))]
    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Focus Reactor")
        .with_title("⏸ Idle")
        .build()?;

    let tray_icon = Rc::new(RefCell::new(tray_icon));

    // メニューイベントハンドラ
    let shared_for_menu = Arc::clone(&shared_state);
    muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
        if event.id == show_app_id {
            if let Ok(mut state) = shared_for_menu.lock() {
                state.should_focus = true;
            }
        } else if event.id == quit_id {
            std::process::exit(0);
        }
    }));

    // eframeアプリを起動
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 180.0])
            .with_min_inner_size([300.0, 180.0])
            .with_transparent(true),
        ..Default::default()
    };

    let shared_for_app = Arc::clone(&shared_state);
    let tray_for_app = Rc::clone(&tray_icon);

    eframe::run_native(
        "Focus Reactor",
        options,
        Box::new(move |cc| {
            // 透明ウィンドウ用のビジュアル設定
            let mut visuals = egui::Visuals::light();
            visuals.window_fill = egui::Color32::TRANSPARENT;
            visuals.panel_fill = egui::Color32::TRANSPARENT;
            cc.egui_ctx.set_visuals(visuals);
            Ok(Box::new(FocusReactorApp::new(
                shared_for_app,
                tray_for_app,
                #[cfg(target_os = "macos")]
                ns_menu_ptr,
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))
}

struct FocusReactorApp {
    state: TimerState,
    work_duration: u64,
    break_duration: u64,
    max_duration: u64,
    last_tick: Instant,
    shared_state: Arc<Mutex<SharedState>>,
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    tray_icon: Rc<RefCell<TrayIcon>>,
    #[cfg(target_os = "macos")]
    macos_sb: Option<MonospaceStatusBar>,
}

impl FocusReactorApp {
    fn new(
        shared_state: Arc<Mutex<SharedState>>,
        tray_icon: Rc<RefCell<TrayIcon>>,
        #[cfg(target_os = "macos")] ns_menu_ptr: *mut std::ffi::c_void,
    ) -> Self {
        #[cfg(target_os = "macos")]
        let macos_sb = objc2_foundation::MainThreadMarker::new().map(|mtm| {
            let sb = MonospaceStatusBar::new(mtm);
            sb.set_ns_menu(ns_menu_ptr);
            sb.set_icon("stopTemplate.png");
            sb
        });

        let app = Self {
            state: TimerState::Idle,
            work_duration: 25 * 60, // 25分
            break_duration: 5 * 60, // 5分
            max_duration: 25 * 60,
            last_tick: Instant::now(),
            shared_state,
            tray_icon,
            #[cfg(target_os = "macos")]
            macos_sb,
        };
        app.update_tray();
        app
    }

    fn update_tray(&self) {
        let text = self.state.get_tray_text(self.work_duration);

        // macOS: カスタム等幅フォントステータスバーを使用
        #[cfg(target_os = "macos")]
        if let Some(ref sb) = self.macos_sb {
            sb.set_title(&text);
            sb.set_icon(self.state.get_icon_name());
        }

        // 他のプラットフォーム: tray-iconのタイトルを更新
        #[cfg(not(target_os = "macos"))]
        if let Ok(tray) = self.tray_icon.try_borrow() {
            tray.set_title(Some(&text));
        }

        if let Ok(mut shared) = self.shared_state.lock() {
            shared.state = self.state;
        }
    }
}

impl eframe::App for FocusReactorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // メニューからのフォーカス要求をチェック
        {
            if let Ok(mut shared) = self.shared_state.lock() {
                if shared.should_focus {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    shared.should_focus = false;
                }
            }
        }

        // タイマー更新（1秒ごと）
        if self.last_tick.elapsed() >= Duration::from_secs(1) {
            self.last_tick = Instant::now();

            match self.state {
                TimerState::Work(t) => {
                    if t > 0 {
                        self.state = TimerState::Work(t - 1);
                    } else {
                        // タイムアップ後は延長モードに移行
                        self.state = TimerState::WorkOvertime(1);
                    }
                }
                TimerState::WorkOvertime(t) => {
                    // 延長時間をカウントアップ
                    self.state = TimerState::WorkOvertime(t + 1);
                }
                TimerState::Break(t) => {
                    if t > 0 {
                        self.state = TimerState::Break(t - 1);
                    } else {
                        self.state = TimerState::Idle;
                    }
                }
                TimerState::Idle => {}
            }

            // トレイアイコンのタイトルを更新
            self.update_tray();
        }

        // 継続的に再描画をリクエスト（タイマー更新のため）
        ctx.request_repaint_after(Duration::from_millis(100));

        // UI描画（半透明の白背景で視認性を確保）
        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(&ctx.style())
                    .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 230)),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);

                    let (status_text, progress, color) = match self.state {
                        TimerState::Idle => {
                            ("SYSTEM IDLE".to_string(), 0.0, egui::Color32::DARK_GRAY)
                        }
                        TimerState::Work(t) => (
                            format!("🍅 WORK SEQUENCE: {:02}:{:02}", t / 60, t % 60),
                            t as f32 / self.max_duration as f32,
                            egui::Color32::from_rgb(255, 80, 80),
                        ),
                        TimerState::WorkOvertime(overtime) => {
                            let total = self.work_duration + overtime;
                            (
                                format!("🔥 OVERTIME: {:02}:{:02}", total / 60, total % 60),
                                1.0,                                  // バーは常に満タン
                                egui::Color32::from_rgb(255, 165, 0), // オレンジ
                            )
                        }
                        TimerState::Break(t) => (
                            format!("☕ COOLING DOWN: {:02}:{:02}", t / 60, t % 60),
                            t as f32 / self.max_duration as f32,
                            egui::Color32::from_rgb(80, 255, 80),
                        ),
                    };

                    ui.label(egui::RichText::new(&status_text).size(18.0).color(color));

                    ui.add_space(15.0);

                    // カスタムプログレスバー（先端に白い丸）
                    let bar_size = egui::vec2(350.0, 30.0);
                    let (rect, _response) = ui.allocate_exact_size(bar_size, egui::Sense::hover());

                    if ui.is_rect_visible(rect) {
                        let painter = ui.painter();

                        // 背景
                        painter.rect_filled(
                            rect,
                            egui::CornerRadius::same(4),
                            egui::Color32::from_gray(200),
                        );

                        // 進捗部分
                        let progress_width = rect.width() * progress;
                        if progress_width > 0.0 {
                            let progress_rect = egui::Rect::from_min_size(
                                rect.min,
                                egui::vec2(progress_width, rect.height()),
                            );
                            painter.rect_filled(progress_rect, egui::CornerRadius::same(4), color);
                        }

                        // // 先端の白い丸
                        // let circle_x = rect.min.x + progress_width;
                        // let circle_y = rect.center().y;
                        // let circle_radius = rect.height() * 0.35;
                        // painter.circle_filled(
                        //     egui::pos2(circle_x, circle_y),
                        //     circle_radius,
                        //     egui::Color32::WHITE,
                        // );
                    }

                    ui.add_space(30.0);

                    // ボタン
                    ui.horizontal(|ui| {
                        ui.add_space(30.0);

                        if ui
                            .add_sized([100.0, 40.0], egui::Button::new("🍅 Pomodoro\n(25m)"))
                            .clicked()
                        {
                            self.state = TimerState::Work(self.work_duration);
                            self.max_duration = self.work_duration;
                            self.last_tick = Instant::now();
                            self.update_tray();
                        }

                        if ui
                            .add_sized([100.0, 40.0], egui::Button::new("☕ Break\n(5m)"))
                            .clicked()
                        {
                            self.state = TimerState::Break(self.break_duration);
                            self.max_duration = self.break_duration;
                            self.last_tick = Instant::now();
                            self.update_tray();
                        }

                        if ui
                            .add_sized([100.0, 40.0], egui::Button::new("🔄 Reset"))
                            .clicked()
                        {
                            self.state = TimerState::Idle;
                            self.update_tray();
                        }
                    });

                    ui.add_space(10.0);
                });
            });

        // Oキーでウィンドウをフォーカス
        if ctx.input(|i| i.key_pressed(egui::Key::O)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }
}
