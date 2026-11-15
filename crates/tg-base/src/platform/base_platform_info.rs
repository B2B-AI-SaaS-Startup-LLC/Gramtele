//! Non-Qt model helpers from base_platform_info.cpp, UTF-16 length limits.
pub fn is_device_model_ok(model: &str) -> bool {
    !model.is_empty() && model.encode_utf16().count() <= 15
}
pub fn simplify_device_model(model: &str) -> String {
    crate::algorithm::clean_and_simplify(&model.replace('_', ""))
}
pub fn simplify_good_device_model(model: &str, remove: &[&str]) -> String {
    let mut result = String::new();
    for word in model.split(' ') {
        if remove.contains(&word.to_lowercase().as_str()) {
            continue;
        }
        if result.is_empty() {
            result = word.to_owned();
        } else if result.encode_utf16().count() + word.encode_utf16().count() + 1 > 32 {
            return result;
        } else {
            result.push(' ');
            result.push_str(word);
        }
    }
    result
}
pub fn product_name_to_device_model(product: &str) -> String {
    if product.starts_with("HP ") {
        simplify_good_device_model(
            product,
            &["notebook", "desktop", "mobile", "workstation", "pc"],
        )
    } else if is_device_model_ok(product) {
        product.to_owned()
    } else {
        String::new()
    }
}
pub fn finalize_device_model(model: &str) -> String {
    let s = model.trim();
    if s.is_empty() {
        "Desktop".into()
    } else {
        s.to_owned()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutdateReason {
    IsOld,
    Is32Bit,
}
