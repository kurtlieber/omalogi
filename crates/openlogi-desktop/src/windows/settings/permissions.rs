//! Permissions settings page: Linux input-device access (udev rules).

use super::{
    IconName, Palette, ParentElement, PermissionStatus, SettingField, SettingGroup, SettingItem,
    SettingPage, SharedString, Styled, div, h_flex, px, rgb, theme,
};
use crate::ui::theme::Typography as _;
use openlogi_permissions as permissions;

pub(super) fn permissions_page() -> SettingPage {
    let page = SettingPage::new(tr!("permissions.permissions"))
        .icon(IconName::Info)
        .resettable(false);

    page.group(SettingGroup::new().item({
        // Description is only shown when access is not yet granted — no noise
        // when everything is already working.
        SettingItem::new(
            tr!("permissions.input_device_access"),
            SettingField::render(move |_, _, cx| {
                let pal = theme::palette(cx);
                let status = permissions::input_device_access();
                let field = gpui_component::v_flex()
                    .gap_1()
                    .child(status_badge(status, pal));
                let hint = match status {
                    PermissionStatus::Denied => {
                        Some(tr!("permissions.linux_input_access_denied_description"))
                    }
                    PermissionStatus::Unknown => {
                        Some(tr!("permissions.linux_input_access_unknown_description"))
                    }
                    PermissionStatus::Granted => None,
                };
                if let Some(text) = hint {
                    field.child(div().text_caption().text_color(pal.text_muted).child(text))
                } else {
                    field
                }
            }),
        )
    }))
}

/// A readable status word with colour retained as a supplemental marker.
fn status_badge(status: PermissionStatus, pal: Palette) -> gpui::Div {
    let (label, color) = match status {
        PermissionStatus::Granted => (tr!("permissions.granted"), theme::STATUS_CONNECTED),
        PermissionStatus::Denied => (tr!("permissions.not_granted"), theme::STATUS_CONNECTING),
        PermissionStatus::Unknown => (tr!("permissions.unknown"), theme::STATUS_OFFLINE),
    };
    badge(label, color, pal)
}

fn badge(label: SharedString, color: u32, pal: Palette) -> gpui::Div {
    h_flex()
        .items_center()
        .gap_1()
        .text_caption()
        .text_color(pal.text_primary)
        .child(div().size(px(6.)).rounded_full().bg(rgb(color)))
        .child(label)
}
