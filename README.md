# Rainbow-Blog

一个基于 Medium 风格的现代化博客平台，使用 Rust + Axum + SurrealDB 构建。

## 特性

- 📝 **Medium 风格的写作体验** - 简洁优雅的编辑器，支持 Markdown 和富文本
- 👥 **完整的社交功能** - 关注、点赞、评论、收藏
- 🏢 **出版物系统** - 创建和管理多作者出版物
- 💰 **会员订阅** - 支持付费内容和创作者收益
- 🔍 **智能推荐** - 基于用户兴趣的个性化推荐
- 📊 **详细统计** - 文章阅读数据和收益分析
- 🎨 **响应式设计** - 完美适配各种设备
- ⚡ **高性能** - 基于 Rust 的极速后端

## 技术栈

- **后端框架**: Rust + Axum
- **数据库**: SurrealDB
- **认证**: JWT
- **缓存**: Redis
- **存储**: MinIO/S3
- **搜索**: SurrealDB 全文搜索

## 快速开始

### 环境要求

- Rust 1.70+
- SurrealDB 1.5.6+
- Redis 6.0+ (可选)
- MinIO 或 S3 兼容存储 (可选)

### 安装步骤

1. 克隆仓库
```bash
git clone https://github.com/rainbow-hub/Rainbow-Blog.git
cd Rainbow-Blog
```

2. 复制环境变量配置
```bash
cp .env.example .env
# 编辑 .env 文件配置数据库和其他服务
```

3. 启动 SurrealDB
```bash
surreal start --log debug --user root --pass root memory
```

4. 初始化数据库
```bash
surreal import --conn http://localhost:8000 --user root --pass root --ns rainbow --db blog schemas/blog_schema.sql
```

5. 运行应用
```bash
cargo run --release
```

应用将在 `http://localhost:3000` 启动

## 项目结构

```
Rainbow-Blog/
├── src/
│   ├── main.rs         # 应用入口
│   ├── config.rs       # 配置管理
│   ├── models/         # 数据模型
│   ├── routes/         # API 路由
│   ├── services/       # 业务逻辑
│   └── utils/          # 工具函数
├── schemas/            # 数据库架构
├── tests/              # 测试文件
└── docs/               # 项目文档
```

## API 文档

主要 API 端点：

- `POST /api/auth/register` - 用户注册
- `POST /api/auth/login` - 用户登录
- `GET /api/articles` - 获取文章列表
- `POST /api/articles` - 创建文章
- `GET /api/articles/:slug` - 获取文章详情
- `POST /api/articles/:slug/clap` - 点赞文章
- `POST /api/articles/:slug/comments` - 发表评论

完整 API 文档请查看 [API.md](docs/API.md)

## 开发指南

### 本地开发

```bash
# 安装依赖
cargo build

# 运行测试
cargo test

# 代码检查
cargo clippy

# 格式化代码
cargo fmt
```

### 数据库迁移

```bash
# 运行迁移
surreal import --conn $DATABASE_URL --ns rainbow --db blog schemas/migrations/*.sql
```

## 部署

### Docker 部署

```bash
docker-compose up -d
```

### 生产环境配置

1. 设置环境变量
2. 配置 Nginx 反向代理
3. 启用 SSL/TLS
4. 配置 CDN
5. 设置监控和日志

详细部署指南请查看 [DEPLOYMENT.md](docs/DEPLOYMENT.md)

## 贡献指南

我们欢迎所有形式的贡献！请查看 [CONTRIBUTING.md](CONTRIBUTING.md) 了解如何参与项目。

## 许可证

本项目采用 MIT 许可证。详见 [LICENSE](LICENSE) 文件。

## 相关项目

- [Rainbow-Auth](../Rainbow-Auth) - 统一认证服务
- [Rainbow-docs](../Rainbow-docs) - 文档管理系统
- [soulcore](../soulcore) - 核心基础设施库

## 联系我们

- GitHub: [Rainbow Hub](https://github.com/rainbow-hub)
- Email: contact@rainbow-hub.com

---

Built with ❤️ by Rainbow Hub Team