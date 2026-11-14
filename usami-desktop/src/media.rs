//! Mídia anexada ao chat (imagem pronta para preview e envio).

use crate::api;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) struct AttachedMedia {
    pub(crate) filename: String,
    pub(crate) mime_type: String,
    pub(crate) data_base64: String,
    pub(crate) preview: Arc<gpui::Image>,
    /// Caminho relativo a `data_dir()` onde o arquivo foi gravado, se já persistido.
    pub(crate) stored_path: Option<String>,
}

impl AttachedMedia {
    pub(crate) fn from_bytes(filename: String, mime_type: &str, bytes: Vec<u8>) -> Self {
        let format = match mime_type {
            "image/jpeg" | "image/jpg" => gpui::ImageFormat::Jpeg,
            "image/webp" => gpui::ImageFormat::Webp,
            "image/gif" => gpui::ImageFormat::Gif,
            "image/bmp" => gpui::ImageFormat::Bmp,
            _ => gpui::ImageFormat::Png,
        };
        let data_base64 = api::base64_encode(&bytes);
        let preview = Arc::new(gpui::Image::from_bytes(format, bytes));
        Self {
            filename,
            mime_type: mime_type.into(),
            data_base64,
            preview,
            stored_path: None,
        }
    }
}
