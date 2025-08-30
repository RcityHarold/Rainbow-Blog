use pulldown_cmark::{html, Options, Parser, Event, Tag, CodeBlockKind};
use syntect::html::{ClassedHTMLGenerator, ClassStyle};
use syntect::parsing::SyntaxSet;
use syntect::highlighting::ThemeSet;
use ammonia::Builder;
use std::collections::{HashMap, HashSet};
use regex::Regex;
use once_cell::sync::Lazy;
use serde::{Serialize, Deserialize};
use maplit::{hashset, hashmap};

static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: Lazy<ThemeSet> = Lazy::new(ThemeSet::load_defaults);

#[derive(Clone)]
pub struct MarkdownProcessor {}

impl Default for MarkdownProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownProcessor {
    pub fn new() -> Self {
        Self {}
    }
    
    fn get_sanitizer() -> Builder<'static> {
        // 配置 HTML 清理器
        let mut sanitizer = Builder::default();
        
        // 允许的标签
        sanitizer.tags(hashset![
            "h1", "h2", "h3", "h4", "h5", "h6",
            "p", "br", "hr",
            "strong", "em", "u", "s", "code",
            "pre", "blockquote",
            "ul", "ol", "li",
            "a", "img",
            "table", "thead", "tbody", "tr", "th", "td",
            "div", "span",
            "sup", "sub"
        ]);

        // 配置标签属性
        let mut tag_attrs = HashMap::new();
        tag_attrs.insert("a", hashset!["href", "title", "target", "rel"]);
        tag_attrs.insert("img", hashset!["src", "alt", "title", "width", "height"]);
        tag_attrs.insert("pre", hashset!["class"]);
        tag_attrs.insert("code", hashset!["class"]);
        tag_attrs.insert("div", hashset!["class"]);
        tag_attrs.insert("span", hashset!["class"]);
        
        sanitizer.tag_attributes(tag_attrs);
        sanitizer
    }

    /// 将 Markdown 转换为 HTML
    pub fn to_html(&self, markdown: &str) -> String {
        // 配置 CommonMark 选项
        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_SMART_PUNCTUATION);

        let parser = Parser::new_ext(markdown, options);
        
        // 处理代码块语法高亮
        let events = self.highlight_code_blocks(parser);
        
        // 转换为 HTML
        let mut html_output = String::new();
        html::push_html(&mut html_output, events.into_iter());
        
        // 清理和安全化 HTML
        let sanitizer = Self::get_sanitizer();
        sanitizer.clean(&html_output).to_string()
    }

    /// 从 Markdown 提取纯文本（用于搜索和摘要）
    pub fn to_text(&self, markdown: &str) -> String {
        let options = Options::empty();
        let parser = Parser::new_ext(markdown, options);
        
        let mut text = String::new();
        for event in parser {
            match event {
                Event::Text(t) => text.push_str(&t),
                Event::Code(t) => text.push_str(&t),
                Event::SoftBreak | Event::HardBreak => text.push(' '),
                _ => {}
            }
        }
        
        // 清理多余的空白
        let whitespace_regex = Regex::new(r"\s+").unwrap();
        whitespace_regex.replace_all(&text, " ").trim().to_string()
    }

    /// 生成文章摘要
    pub fn generate_excerpt(&self, markdown: &str, max_length: usize) -> String {
        let text = self.to_text(markdown);
        
        if text.len() <= max_length {
            return text;
        }
        
        // 在最接近的单词边界处截断
        let mut end = max_length;
        while end > 0 && !text.chars().nth(end).map_or(false, |c| c.is_whitespace()) {
            end -= 1;
        }
        
        if end == 0 {
            end = max_length;
        }
        
        format!("{}...", text.chars().take(end).collect::<String>().trim())
    }

    /// 提取文章中的图片
    pub fn extract_images(&self, markdown: &str) -> Vec<String> {
        let options = Options::empty();
        let parser = Parser::new_ext(markdown, options);
        
        let mut images = Vec::new();
        for event in parser {
            if let Event::Start(Tag::Image(_, url, _)) = event {
                images.push(url.to_string());
            }
        }
        
        images
    }

    /// 提取第一张图片作为封面
    pub fn extract_cover_image(&self, markdown: &str) -> Option<String> {
        self.extract_images(markdown).into_iter().next()
    }

    /// 处理代码块语法高亮
    fn highlight_code_blocks<'a>(&self, parser: Parser<'a, 'a>) -> Vec<Event<'a>> {
        let mut events = Vec::new();
        let mut in_code_block = false;
        let mut code_buffer = String::new();
        let mut language = String::new();

        for event in parser {
            match event {
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                    in_code_block = true;
                    language = lang.to_string();
                    code_buffer.clear();
                    // 不添加任何事件，等待代码块结束
                }
                Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(_))) => {
                    in_code_block = false;
                    let highlighted = self.highlight_code(&code_buffer, &language);
                    events.push(Event::Html(highlighted.into()));
                }
                Event::Text(text) if in_code_block => {
                    code_buffer.push_str(&text);
                    // 不添加任何事件
                }
                _ if !in_code_block => events.push(event),
                _ => {} // 在代码块内部的其他事件被忽略
            }
        }

        events
    }

    /// 语法高亮代码
    fn highlight_code(&self, code: &str, language: &str) -> String {
        let syntax = SYNTAX_SET.find_syntax_by_token(language)
            .or_else(|| SYNTAX_SET.find_syntax_by_extension(language))
            .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

        let mut html_generator = ClassedHTMLGenerator::new_with_class_style(
            syntax,
            &SYNTAX_SET,
            ClassStyle::Spaced,
        );

        for line in code.lines() {
            html_generator.parse_html_for_line_which_includes_newline(line);
        }

        format!(
            r#"<pre class="highlight"><code class="language-{}">{}</code></pre>"#,
            language,
            html_generator.finalize()
        )
    }

    /// 估算阅读时间（分钟）
    pub fn estimate_reading_time(&self, markdown: &str) -> i32 {
        let word_count = self.count_words(markdown);
        let words_per_minute = 200;
        std::cmp::max(1, (word_count as i32 + words_per_minute - 1) / words_per_minute)
    }
    
    /// 计算字数
    pub fn count_words(&self, markdown: &str) -> usize {
        let text = self.to_text(markdown);
        text.split_whitespace().count()
    }

    /// 提取文章目录
    pub fn extract_toc(&self, markdown: &str) -> Vec<TocItem> {
        let options = Options::empty();
        let parser = Parser::new_ext(markdown, options);
        
        let mut toc = Vec::new();
        let mut current_text = String::new();
        let mut in_heading = false;
        let mut heading_level = 0;

        for event in parser {
            match event {
                Event::Start(Tag::Heading(level, _, _)) => {
                    in_heading = true;
                    heading_level = level as u8;
                    current_text.clear();
                }
                Event::End(Tag::Heading(_, _, _)) => {
                    if in_heading && !current_text.is_empty() {
                        let id = self.generate_heading_id(&current_text);
                        toc.push(TocItem {
                            level: heading_level,
                            title: current_text.clone(),
                            id,
                        });
                    }
                    in_heading = false;
                }
                Event::Text(text) if in_heading => {
                    current_text.push_str(&text);
                }
                _ => {}
            }
        }

        toc
    }

    /// 为标题生成ID
    fn generate_heading_id(&self, title: &str) -> String {
        title
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() || c == ' ' { c } else { ' ' })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("-")
    }

    /// 提取内容预览（用于付费内容）
    pub fn extract_preview(&self, markdown: &str, html: &str, percentage: u8) -> (String, String) {
        let percentage = percentage.min(100);
        
        if percentage >= 100 {
            return (markdown.to_string(), html.to_string());
        }
        
        // 按段落分割内容
        let paragraphs: Vec<&str> = markdown.split("\n\n").collect();
        let total_paragraphs = paragraphs.len();
        let preview_paragraphs = (total_paragraphs * percentage as usize / 100).max(1);
        
        let preview_markdown = paragraphs
            .iter()
            .take(preview_paragraphs)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n");
        
        let preview_html = self.to_html(&preview_markdown);
        
        (preview_markdown, preview_html)
    }

    /// 在 Markdown 中添加目录链接
    pub fn add_toc_links(&self, markdown: &str) -> String {
        let toc = self.extract_toc(markdown);
        let toc_map: HashMap<String, String> = toc
            .into_iter()
            .map(|item| (item.title.clone(), item.id))
            .collect();

        let heading_regex = Regex::new(r"^(#{1,6})\s+(.+)$").unwrap();
        
        markdown
            .lines()
            .map(|line| {
                if let Some(captures) = heading_regex.captures(line) {
                    let hashes = &captures[1];
                    let title = &captures[2];
                    
                    if let Some(id) = toc_map.get(title) {
                        format!("{} {} {{#{}}}", hashes, title, id)
                    } else {
                        line.to_string()
                    }
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TocItem {
    pub level: u8,
    pub title: String,
    pub id: String,
}

// 便利宏
#[macro_export]
macro_rules! hashset {
    ($($item:expr),*) => {
        {
            let mut set = std::collections::HashSet::new();
            $(set.insert($item);)*
            set
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_to_html() {
        let processor = MarkdownProcessor::new();
        
        let markdown = "# Hello World\n\nThis is **bold** text.";
        let html = processor.to_html(markdown);
        
        assert!(html.contains("<h1>Hello World</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn test_extract_text() {
        let processor = MarkdownProcessor::new();
        
        let markdown = "# Hello World\n\nThis is **bold** text with `code`.";
        let text = processor.to_text(markdown);
        
        assert_eq!(text, "Hello World This is bold text with code.");
    }

    #[test]
    fn test_generate_excerpt() {
        let processor = MarkdownProcessor::new();
        
        let markdown = "This is a very long article that should be truncated at some reasonable point.";
        let excerpt = processor.generate_excerpt(markdown, 50);
        
        assert!(excerpt.len() <= 53); // 50 + "..."
        assert!(excerpt.ends_with("..."));
    }

    #[test]
    fn test_extract_toc() {
        let processor = MarkdownProcessor::new();
        
        let markdown = "# Chapter 1\n\n## Section 1.1\n\n### Subsection 1.1.1\n\n# Chapter 2";
        let toc = processor.extract_toc(markdown);
        
        assert_eq!(toc.len(), 4);
        assert_eq!(toc[0].level, 1);
        assert_eq!(toc[0].title, "Chapter 1");
        assert_eq!(toc[1].level, 2);
        assert_eq!(toc[1].title, "Section 1.1");
    }
}