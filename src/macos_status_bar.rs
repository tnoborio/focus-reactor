//! macOS専用: 等幅フォントでステータスバーにテキストを表示するモジュール

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_app_kit::{
    NSFont, NSImage, NSStatusBar, NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::{MainThreadMarker, NSAttributedString, NSDictionary, NSString};

/// 等幅フォントを使用するmacOSステータスバーアイテム
pub struct MonospaceStatusBar {
    status_item: Retained<NSStatusItem>,
    mtm: MainThreadMarker,
}

impl MonospaceStatusBar {
    /// 新しいステータスバーアイテムを作成
    pub fn new(mtm: MainThreadMarker) -> Self {
        let status_bar = NSStatusBar::systemStatusBar();
        let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);

        Self {
            status_item,
            mtm,
        }
    }

    /// 外部のNSMenuポインタ（mudaのContextMenu::ns_menu()等）をステータスアイテムに設定
    pub fn set_ns_menu(&self, ns_menu_ptr: *mut std::ffi::c_void) {
        unsafe {
            let menu_obj: &AnyObject = &*(ns_menu_ptr as *const AnyObject);
            let _: () = msg_send![&self.status_item, setMenu: menu_obj];
        }
    }

    /// アイコンを設定
    pub fn set_icon(&self, icon_name: &str) {
        if let Some(button) = self.status_item.button(self.mtm) {
            let path = format!(
                "{}/assets/{}",
                std::env::current_dir().unwrap().display(),
                icon_name
            );
            let ns_string_path = NSString::from_str(&path);

            unsafe {
                let cls = class!(NSImage);
                let img_alloc: *mut NSImage = msg_send![cls, alloc];
                let img_init: *mut NSImage =
                    msg_send![img_alloc, initByReferencingFile: &*ns_string_path];

                if let Some(img) = Retained::from_raw(img_init) {
                    img.setTemplate(true); // Template Imageとして扱う（ダークモード対応）
                    button.setImage(Some(&img));
                }
            }
        }
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
