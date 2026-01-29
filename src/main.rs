use anyhow::Result;
use eframe::egui;
use muda::{Menu, MenuId, MenuItem};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tray_icon::{TrayIcon, TrayIconBuilder};

// タイマーの状態
#[derive(Clone, Copy, PartialEq, Debug)]
enum TimerState {
    Idle,
    Work(u64),  // 残り秒数
    Break(u64), // 残り秒数
}

impl TimerState {
    fn get_tray_text(&self) -> String {
        match self {
            TimerState::Idle => "⏸ Idle".to_string(),
            TimerState::Work(t) => format!("🍅 {:02}:{:02}", t / 60, t % 60),
            TimerState::Break(t) => format!("☕ {:02}:{:02}", t / 60, t % 60),
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

    // トレイアイコン作成（空のアイコン - macOSではテキストのみ表示可能）
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
            .with_min_inner_size([300.0, 180.0]),
        ..Default::default()
    };

    let shared_for_app = Arc::clone(&shared_state);
    let tray_for_app = Rc::clone(&tray_icon);

    eframe::run_native(
        "Focus Reactor",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            Ok(Box::new(FocusReactorApp::new(shared_for_app, tray_for_app)))
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
    tray_icon: Rc<RefCell<TrayIcon>>,
}

impl FocusReactorApp {
    fn new(shared_state: Arc<Mutex<SharedState>>, tray_icon: Rc<RefCell<TrayIcon>>) -> Self {
        Self {
            state: TimerState::Idle,
            work_duration: 25 * 60, // 25分
            break_duration: 5 * 60, // 5分
            max_duration: 25 * 60,
            last_tick: Instant::now(),
            shared_state,
            tray_icon,
        }
    }

    fn update_tray(&self) {
        if let Ok(tray) = self.tray_icon.try_borrow() {
            tray.set_title(Some(&self.state.get_tray_text()));
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
                TimerState::Work(t) | TimerState::Break(t) => {
                    if t > 0 {
                        let next_time = t - 1;
                        self.state = if matches!(self.state, TimerState::Work(_)) {
                            TimerState::Work(next_time)
                        } else {
                            TimerState::Break(next_time)
                        };
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

        // UI描画
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).fill(egui::Color32::WHITE))
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
                        TimerState::Break(t) => (
                            format!("☕ COOLING DOWN: {:02}:{:02}", t / 60, t % 60),
                            t as f32 / self.max_duration as f32,
                            egui::Color32::from_rgb(80, 255, 80),
                        ),
                    };

                    ui.label(egui::RichText::new(&status_text).size(18.0).color(color));

                    ui.add_space(15.0);

                    // プログレスバー
                    let progress_bar =
                        egui::ProgressBar::new(progress)
                            .fill(color)
                            .animate(matches!(
                                self.state,
                                TimerState::Work(_) | TimerState::Break(_)
                            ));
                    ui.add_sized([350.0, 30.0], progress_bar);

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
