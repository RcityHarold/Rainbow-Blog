use std::io::Cursor;
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};

/// 图片格式枚举
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Webp,
    Gif,
}

impl ImageFormat {
    /// 从MIME类型获取图片格式
    pub fn from_mime_type(mime_type: &str) -> Option<Self> {
        match mime_type {
            "image/jpeg" | "image/jpg" => Some(Self::Jpeg),
            "image/png" => Some(Self::Png),
            "image/webp" => Some(Self::Webp),
            "image/gif" => Some(Self::Gif),
            _ => None,
        }
    }
    
    /// 获取MIME类型
    pub fn to_mime_type(&self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Webp => "image/webp",
            Self::Gif => "image/gif",
        }
    }
    
    /// 获取文件扩展名
    pub fn to_extension(&self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Gif => "gif",
        }
    }
}

/// 图片尺寸
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageDimensions {
    pub width: u32,
    pub height: u32,
}

/// 图片元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageMetadata {
    pub format: ImageFormat,
    pub dimensions: ImageDimensions,
    pub file_size: usize,
    pub has_transparency: bool,
}

/// 图片处理配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageProcessConfig {
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub quality: Option<u8>, // 1-100, 仅用于JPEG
    pub format: Option<ImageFormat>,
    pub maintain_aspect_ratio: bool,
}

impl Default for ImageProcessConfig {
    fn default() -> Self {
        Self {
            max_width: None,
            max_height: None,
            quality: Some(85),
            format: None,
            maintain_aspect_ratio: true,
        }
    }
}

/// 图片处理工具
pub struct ImageProcessor;

impl ImageProcessor {
    /// 创建新的图片处理器实例
    pub fn new() -> Self {
        Self
    }

    /// 检查数据是否为有效图片
    pub fn is_valid_image(&self, data: &[u8]) -> bool {
        Self::detect_format(data).is_ok()
    }

    /// 获取图片尺寸（实例方法包装）
    pub fn get_dimensions(&self, data: &[u8]) -> Result<ImageDimensions, String> {
        Self::get_image_dimensions_internal(data)
    }

    /// 获取图片尺寸（原有的静态方法重命名）
    pub fn get_image_dimensions(data: &[u8]) -> Result<(u32, u32), String> {
        let dims = Self::get_image_dimensions_internal(data)?;
        Ok((dims.width, dims.height))
    }

    /// 检测图片格式
    pub fn detect_format(data: &[u8]) -> Result<ImageFormat, String> {
        if data.len() < 4 {
            return Err("数据太短，无法检测格式".to_string());
        }

        // 检查文件头
        match &data[0..4] {
            [0xFF, 0xD8, 0xFF, _] => Ok(ImageFormat::Jpeg),
            [0x89, 0x50, 0x4E, 0x47] => Ok(ImageFormat::Png),
            [0x47, 0x49, 0x46, _] => Ok(ImageFormat::Gif),
            _ => {
                // 检查WebP
                if data.len() >= 12 
                    && &data[0..4] == b"RIFF" 
                    && &data[8..12] == b"WEBP" {
                    Ok(ImageFormat::Webp)
                } else {
                    Err("不支持的图片格式".to_string())
                }
            }
        }
    }

    /// 获取图片尺寸（简化版本，实际应该使用image crate）
    pub fn get_image_dimensions_internal(data: &[u8]) -> Result<ImageDimensions, String> {
        let format = Self::detect_format(data)?;

        match format {
            ImageFormat::Png => Self::get_png_dimensions(data),
            ImageFormat::Jpeg => Self::get_jpeg_dimensions(data),
            ImageFormat::Gif => Self::get_gif_dimensions(data),
            ImageFormat::Webp => Self::get_webp_dimensions(data),
        }
    }

    /// 获取PNG尺寸
    fn get_png_dimensions(data: &[u8]) -> Result<ImageDimensions, String> {
        if data.len() < 24 {
            return Err("PNG数据不完整".to_string());
        }
        
        // PNG IHDR块在第8-24字节
        let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        
        Ok(ImageDimensions { width, height })
    }

    /// 获取JPEG尺寸（简化版本）
    fn get_jpeg_dimensions(data: &[u8]) -> Result<ImageDimensions, String> {
        let mut i = 0;
        while i < data.len() - 1 {
            if data[i] == 0xFF {
                let marker = data[i + 1];
                if marker >= 0xC0 && marker <= 0xC3 {
                    // SOF0-SOF3 markers
                    if i + 9 < data.len() {
                        let height = u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32;
                        let width = u16::from_be_bytes([data[i + 7], data[i + 8]]) as u32;
                        return Ok(ImageDimensions { width, height });
                    }
                }
                i += 2;
            } else {
                i += 1;
            }
        }
        Err("无法找到JPEG尺寸信息".to_string())
    }

    /// 获取GIF尺寸
    fn get_gif_dimensions(data: &[u8]) -> Result<ImageDimensions, String> {
        if data.len() < 10 {
            return Err("GIF数据不完整".to_string());
        }
        
        let width = u16::from_le_bytes([data[6], data[7]]) as u32;
        let height = u16::from_le_bytes([data[8], data[9]]) as u32;
        
        Ok(ImageDimensions { width, height })
    }

    /// 获取WebP尺寸（简化版本）
    fn get_webp_dimensions(data: &[u8]) -> Result<ImageDimensions, String> {
        if data.len() < 30 {
            return Err("WebP数据不完整".to_string());
        }
        
        // 简化的WebP VP8解析
        if &data[12..16] == b"VP8 " {
            if data.len() >= 30 {
                let width = (u16::from_le_bytes([data[26], data[27]]) & 0x3FFF) as u32;
                let height = (u16::from_le_bytes([data[28], data[29]]) & 0x3FFF) as u32;
                return Ok(ImageDimensions { width, height });
            }
        }
        
        Err("不支持的WebP格式".to_string())
    }

    /// 获取图片元数据
    pub fn get_metadata(data: &[u8]) -> Result<ImageMetadata, String> {
        let format = Self::detect_format(data)?;
        let dimensions = Self::get_image_dimensions_internal(data)?;
        let has_transparency = Self::has_transparency(&format, data);
        
        Ok(ImageMetadata {
            format,
            dimensions,
            file_size: data.len(),
            has_transparency,
        })
    }

    /// 检查图片是否有透明度
    fn has_transparency(format: &ImageFormat, _data: &[u8]) -> bool {
        match format {
            ImageFormat::Png => true,  // PNG支持透明度
            ImageFormat::Gif => true,  // GIF支持透明度
            ImageFormat::Webp => true, // WebP支持透明度
            ImageFormat::Jpeg => false, // JPEG不支持透明度
        }
    }

    /// 计算缩放后的尺寸
    pub fn calculate_resize_dimensions(
        original: &ImageDimensions,
        config: &ImageProcessConfig,
    ) -> ImageDimensions {
        let mut new_width = original.width;
        let mut new_height = original.height;

        // 应用最大宽度限制
        if let Some(max_width) = config.max_width {
            if new_width > max_width {
                if config.maintain_aspect_ratio {
                    let ratio = max_width as f64 / new_width as f64;
                    new_width = max_width;
                    new_height = (new_height as f64 * ratio) as u32;
                } else {
                    new_width = max_width;
                }
            }
        }

        // 应用最大高度限制
        if let Some(max_height) = config.max_height {
            if new_height > max_height {
                if config.maintain_aspect_ratio {
                    let ratio = max_height as f64 / new_height as f64;
                    new_height = max_height;
                    new_width = (new_width as f64 * ratio) as u32;
                } else {
                    new_height = max_height;
                }
            }
        }

        ImageDimensions {
            width: new_width,
            height: new_height,
        }
    }

    /// 验证图片数据
    pub fn validate_image(data: &[u8], max_size: Option<usize>) -> Result<(), String> {
        // 检查文件大小
        if let Some(max) = max_size {
            if data.len() > max {
                return Err(format!("图片文件过大: {} bytes (最大: {} bytes)", data.len(), max));
            }
        }

        // 检查格式
        let format = Self::detect_format(data)?;
        
        // 检查尺寸
        let dimensions = Self::get_image_dimensions_internal(data)?;
        
        // 验证尺寸合理性
        if dimensions.width == 0 || dimensions.height == 0 {
            return Err("图片尺寸无效".to_string());
        }
        
        if dimensions.width > 10000 || dimensions.height > 10000 {
            return Err("图片尺寸过大".to_string());
        }

        Ok(())
    }

    /// 从Base64解码图片数据
    pub fn decode_base64(base64_data: &str) -> Result<Vec<u8>, String> {
        // 移除数据URL前缀（如果存在）
        let clean_data = if base64_data.starts_with("data:") {
            if let Some(comma_pos) = base64_data.find(',') {
                &base64_data[comma_pos + 1..]
            } else {
                return Err("无效的数据URL格式".to_string());
            }
        } else {
            base64_data
        };

        general_purpose::STANDARD
            .decode(clean_data)
            .map_err(|e| format!("Base64解码失败: {}", e))
    }

    /// 将图片数据编码为Base64
    pub fn encode_base64(data: &[u8], format: &ImageFormat) -> String {
        let encoded = general_purpose::STANDARD.encode(data);
        format!("data:{};base64,{}", format.to_mime_type(), encoded)
    }

    /// 生成图片缩略图（简化版本，返回原始数据）
    /// 在实际项目中，应该使用image crate来进行真正的图片处理
    pub fn generate_thumbnail(
        data: &[u8],
        config: &ImageProcessConfig,
    ) -> Result<Vec<u8>, String> {
        // 验证输入数据
        Self::validate_image(data, None)?;
        
        let metadata = Self::get_metadata(data)?;
        let new_dimensions = Self::calculate_resize_dimensions(&metadata.dimensions, config);
        
        // 如果尺寸没有变化，直接返回原始数据
        if new_dimensions == metadata.dimensions {
            return Ok(data.to_vec());
        }
        
        // TODO: 实际的图片缩放处理
        // 这里应该使用image crate进行真正的图片处理
        // 现在返回原始数据作为占位符
        
        tracing::warn!(
            "图片缩放功能尚未实现，返回原始图片。原始尺寸: {}x{}, 目标尺寸: {}x{}",
            metadata.dimensions.width,
            metadata.dimensions.height,
            new_dimensions.width,
            new_dimensions.height
        );
        
        Ok(data.to_vec())
    }

    /// 优化图片（简化版本）
    pub fn optimize_image(
        data: &[u8],
        config: &ImageProcessConfig,
    ) -> Result<Vec<u8>, String> {
        Self::generate_thumbnail(data, config)
    }
}

/// 图片工具函数
pub mod utils {
    use super::*;

    /// 检查文件是否为支持的图片格式
    pub fn is_supported_image_format(mime_type: &str) -> bool {
        ImageFormat::from_mime_type(mime_type).is_some()
    }

    /// 获取推荐的图片质量设置
    pub fn get_recommended_quality(format: &ImageFormat, file_size: usize) -> u8 {
        match format {
            ImageFormat::Jpeg => {
                if file_size > 1_000_000 {
                    75 // 大文件使用较低质量
                } else if file_size > 500_000 {
                    80 // 中等文件
                } else {
                    85 // 小文件可以使用较高质量
                }
            }
            _ => 85, // 其他格式的默认质量
        }
    }

    /// 生成图片文件名
    pub fn generate_filename(prefix: &str, format: &ImageFormat) -> String {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        format!("{}_{}.{}", prefix, timestamp, format.to_extension())
    }

    /// 计算图片的宽高比
    pub fn calculate_aspect_ratio(dimensions: &ImageDimensions) -> f64 {
        dimensions.width as f64 / dimensions.height as f64
    }

    /// 检查图片是否为横向
    pub fn is_landscape(dimensions: &ImageDimensions) -> bool {
        dimensions.width > dimensions.height
    }

    /// 检查图片是否为正方形
    pub fn is_square(dimensions: &ImageDimensions) -> bool {
        dimensions.width == dimensions.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_detection() {
        // PNG 格式检测
        let png_header = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(ImageProcessor::detect_format(&png_header).unwrap(), ImageFormat::Png);

        // JPEG 格式检测
        let jpeg_header = vec![0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(ImageProcessor::detect_format(&jpeg_header).unwrap(), ImageFormat::Jpeg);

        // GIF 格式检测
        let gif_header = vec![0x47, 0x49, 0x46, 0x38];
        assert_eq!(ImageProcessor::detect_format(&gif_header).unwrap(), ImageFormat::Gif);
    }

    #[test]
    fn test_dimensions_calculation() {
        let original = ImageDimensions { width: 1920, height: 1080 };
        let config = ImageProcessConfig {
            max_width: Some(800),
            max_height: Some(600),
            maintain_aspect_ratio: true,
            ..Default::default()
        };

        let new_dims = ImageProcessor::calculate_resize_dimensions(&original, &config);
        
        // 应该按比例缩放到最大宽度
        assert_eq!(new_dims.width, 800);
        assert_eq!(new_dims.height, 450); // 保持16:9比例
    }

    #[test]
    fn test_base64_encoding_decoding() {
        let test_data = b"test image data";
        let format = ImageFormat::Png;
        
        let encoded = ImageProcessor::encode_base64(test_data, &format);
        assert!(encoded.starts_with("data:image/png;base64,"));
        
        let decoded = ImageProcessor::decode_base64(&encoded).unwrap();
        assert_eq!(decoded, test_data);
    }

    #[test]
    fn test_utils() {
        assert!(utils::is_supported_image_format("image/png"));
        assert!(utils::is_supported_image_format("image/jpeg"));
        assert!(!utils::is_supported_image_format("text/plain"));

        let dims = ImageDimensions { width: 1920, height: 1080 };
        assert!(utils::is_landscape(&dims));
        assert!(!utils::is_square(&dims));
        assert_eq!(utils::calculate_aspect_ratio(&dims), 1920.0 / 1080.0);
    }
}
