//! macOS専用: 等幅フォントでステータスバーにテキストを表示するモジュール

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{msg_send, sel};
use objc2_app_kit::{
    NSFont, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::{MainThreadMarker, NSAttributedString, NSDictionary, NSString};
use std::sync::Arc;

/// メニューアクションのコールバック型
pub type MenuCallback = Arc<dyn Fn() + Send + Sync>;

/// 等幅フォントを使用するmacOSステータスバーアイテム
pub struct MonospaceStatusBar {
    status_item: Retained<NSStatusItem>,
    mtm: MainThreadMarker,
    #[allow(dead_code)]
    menu: Option<Retained<NSMenu>>,
}

impl MonospaceStatusBar {
    /// 新しいステータスバーアイテムを作成
    pub fn new(mtm: MainThreadMarker) -> Self {
        let status_bar = NSStatusBar::systemStatusBar();
        let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);

        Self {
            status_item,
            mtm,
            menu: None,
        }
    }

    /// メニューを設定（「アプリを表示」と「終了」）
    pub fn set_menu(&mut self) {
        let menu = unsafe { NSMenu::new(self.mtm) };

        // 「アプリを表示」メニューアイテム
        let show_title = NSString::from_str("アプリを表示");
        let show_key = NSString::from_str("o");
        let show_item = unsafe {
            let item = NSMenuItem::new(self.mtm);
            item.setTitle(&show_title);
            item.setAction(Some(sel!(showApp:)));
            item.setKeyEquivalent(&show_key);
            item
        };
        unsafe { menu.addItem(&show_item) };

        // 「終了」メニューアイテム
        let quit_title = NSString::from_str("終了");
        let quit_key = NSString::from_str("q");
        let quit_item = unsafe {
            let item = NSMenuItem::new(self.mtm);
            item.setTitle(&quit_title);
            item.setAction(Some(sel!(terminate:)));
            item.setKeyEquivalent(&quit_key);
            item
        };
        unsafe { menu.addItem(&quit_item) };

        // ステータスアイテムにメニューを設定
        unsafe {
            let _: () = msg_send![&self.status_item, setMenu: &*menu];
        }

        self.menu = Some(menu);
    }

    /// 等幅フォントでタイトルを設定
    pub fn set_title(&self, title: &str) {
        if let Some(button) = self.status_item.button(self.mtm) {
            let ns_string = NSString::from_str(title);
            let font = NSFont::monospacedSystemFontOfSize_weight(12.0, 0.0);

            // NSDictionaryを正しい型で作成
            let key = NSString::from_str("NSFont");
            let keys: &[&NSString] = &[&key];
            // fontをAnyObjectにキャストする
            let font_ref: &AnyObject = unsafe { std::mem::transmute(&*font) };
            let objects: &[&AnyObject] = &[font_ref];
            let attrs: objc2::rc::Retained<NSDictionary<NSString, AnyObject>> =
                NSDictionary::from_slices(keys, objects);

            let attributed_string =
                unsafe { NSAttributedString::new_with_attributes(&ns_string, &attrs) };

            // Retainedを直接使用
            button.setAttributedTitle(&attributed_string);
        }
    }

    /// ステータスアイテムを削除
    pub fn remove(&self) {
        let status_bar = NSStatusBar::systemStatusBar();
        status_bar.removeStatusItem(&self.status_item);
    }
}

impl Drop for MonospaceStatusBar {
    fn drop(&mut self) {
        self.remove();
    }
}
