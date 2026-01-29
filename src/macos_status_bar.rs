//! macOS専用: 等幅フォントでステータスバーにテキストを表示するモジュール

use objc2::runtime::AnyObject;
use objc2_app_kit::{NSFont, NSStatusBar, NSStatusItem, NSVariableStatusItemLength};
use objc2_foundation::{MainThreadMarker, NSAttributedString, NSDictionary, NSString};

/// 等幅フォントを使用するmacOSステータスバーアイテム
pub struct MonospaceStatusBar {
    status_item: objc2::rc::Retained<NSStatusItem>,
    mtm: MainThreadMarker,
}

impl MonospaceStatusBar {
    /// 新しいステータスバーアイテムを作成
    pub fn new(mtm: MainThreadMarker) -> Self {
        let status_bar = NSStatusBar::systemStatusBar();
        let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);

        Self { status_item, mtm }
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
