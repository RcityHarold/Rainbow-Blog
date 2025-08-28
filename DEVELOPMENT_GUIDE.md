# Rainbow-Blog 开发文档

## 项目概述

Rainbow-Blog 是一个基于 Medium 风格的博客系统，完全复刻 Medium 的核心功能。本项目采用与 Rainbow-docs 相同的技术栈，基于 Rust + Axum 框架构建，使用 SurrealDB 作为数据库，通过 soulcore 进行数据库操作。

## 技术栈

### 后端技术栈
- **编程语言**: Rust
- **Web 框架**: Axum 0.6
- **数据库**: SurrealDB 1.5.6 (通过 HTTP 协议)
- **数据库操作**: soulcore (Rainbow Hub 核心库)
- **认证**: JWT (jsonwebtoken 8.3)
- **密码加密**: Argon2
- **异步运行时**: Tokio 1.28
- **序列化**: Serde 1.0
- **日志**: Tracing 0.1
- **HTTP 客户端**: Reqwest 0.11

### 前端技术栈（待定）
- 建议使用 React/Vue.js/Svelte 等现代前端框架
- 需要支持 Markdown 编辑器
- 响应式设计

## 核心功能模块

### 1. 用户系统
- 用户注册/登录（集成 Rainbow-Auth）
- 个人资料管理
- 作者主页
- 关注/粉丝系统
- 用户设置

### 2. 文章系统
- 文章创建/编辑（Markdown + 富文本）
- 草稿自动保存
- 文章发布/取消发布
- 文章版本控制
- 文章系列（Series）
- 响应式图片上传

### 3. 互动功能
- 评论系统
- 点赞（Claps，可多次点赞）
- 书签收藏
- 分享功能
- 高亮和笔记

### 4. 推荐系统
- 个性化推荐
- 热门文章
- 基于标签的推荐
- 相关文章推荐

### 5. 搜索系统
- 全文搜索
- 标签搜索
- 作者搜索
- 高级筛选

### 6. 出版物（Publications）
- 创建出版物
- 管理编辑和作者
- 出版物主页
- 投稿系统

### 7. 会员系统
- 付费订阅
- 会员专属内容
- 作者收益系统

## 项目结构

```
Rainbow-Blog/
├── Cargo.toml              # 项目依赖配置
├── Cargo.lock              # 依赖锁定文件
├── README.md               # 项目说明
├── .env.example            # 环境变量示例
├── build.sh                # 构建脚本
├── src/
│   ├── main.rs            # 应用入口
│   ├── config.rs          # 配置管理
│   ├── error.rs           # 错误处理
│   ├── state.rs           # 应用状态
│   ├── models/            # 数据模型
│   │   ├── mod.rs
│   │   ├── user.rs        # 用户模型
│   │   ├── article.rs     # 文章模型
│   │   ├── comment.rs     # 评论模型
│   │   ├── publication.rs # 出版物模型
│   │   ├── tag.rs         # 标签模型
│   │   ├── clap.rs        # 点赞模型
│   │   ├── bookmark.rs    # 书签模型
│   │   ├── follow.rs      # 关注模型
│   │   ├── series.rs      # 系列模型
│   │   └── subscription.rs # 订阅模型
│   ├── routes/            # 路由处理
│   │   ├── mod.rs
│   │   ├── auth.rs        # 认证路由
│   │   ├── articles.rs    # 文章路由
│   │   ├── users.rs       # 用户路由
│   │   ├── comments.rs    # 评论路由
│   │   ├── publications.rs # 出版物路由
│   │   ├── tags.rs        # 标签路由
│   │   ├── search.rs      # 搜索路由
│   │   ├── stats.rs       # 统计路由
│   │   └── media.rs       # 媒体上传路由
│   ├── services/          # 业务逻辑
│   │   ├── mod.rs
│   │   ├── auth.rs        # 认证服务
│   │   ├── article.rs     # 文章服务
│   │   ├── user.rs        # 用户服务
│   │   ├── comment.rs     # 评论服务
│   │   ├── publication.rs # 出版物服务
│   │   ├── recommendation.rs # 推荐服务
│   │   ├── search.rs      # 搜索服务
│   │   ├── notification.rs # 通知服务
│   │   ├── email.rs       # 邮件服务
│   │   └── media.rs       # 媒体处理服务
│   └── utils/             # 工具函数
│       ├── mod.rs
│       ├── markdown.rs    # Markdown 处理
│       ├── slug.rs        # URL slug 生成
│       ├── image.rs       # 图片处理
│       └── cache.rs       # 缓存工具
├── schemas/               # 数据库架构
│   └── blog_schema.sql    # SurrealDB 表结构
├── tests/                 # 测试文件
├── examples/              # 示例代码
└── docs/                  # 项目文档

```

## 数据库设计

### 核心表结构

1. **用户表** (user)
   - 基本信息：用户名、邮箱、头像
   - 个人简介、社交链接
   - 统计信息：文章数、粉丝数、关注数

2. **文章表** (article)
   - 标题、内容、摘要
   - 作者信息
   - 发布状态、发布时间
   - 统计信息：阅读数、点赞数、评论数
   - SEO 信息

3. **评论表** (comment)
   - 评论内容
   - 作者信息
   - 回复关系
   - 点赞数

4. **标签表** (tag)
   - 标签名称
   - 使用次数
   - 相关文章数

5. **出版物表** (publication)
   - 出版物信息
   - 编辑和作者列表
   - 设置和主题

6. **点赞表** (clap)
   - 用户和文章关联
   - 点赞次数（最多50次）

7. **关注表** (follow)
   - 关注者和被关注者
   - 关注时间

8. **书签表** (bookmark)
   - 用户收藏的文章
   - 收藏分类

9. **系列表** (series)
   - 系列名称和描述
   - 包含的文章列表

10. **订阅表** (subscription)
    - 订阅类型和价格
    - 订阅用户和作者/出版物

## API 设计

### RESTful API 端点

#### 认证相关
- `POST /api/auth/register` - 用户注册
- `POST /api/auth/login` - 用户登录
- `POST /api/auth/logout` - 用户登出
- `POST /api/auth/refresh` - 刷新 Token
- `GET /api/auth/me` - 获取当前用户信息

#### 用户相关
- `GET /api/users/:username` - 获取用户信息
- `PUT /api/users/:username` - 更新用户信息
- `GET /api/users/:username/articles` - 获取用户文章
- `GET /api/users/:username/followers` - 获取粉丝列表
- `GET /api/users/:username/following` - 获取关注列表
- `POST /api/users/:username/follow` - 关注用户
- `DELETE /api/users/:username/follow` - 取消关注

#### 文章相关
- `GET /api/articles` - 获取文章列表
- `POST /api/articles` - 创建文章
- `GET /api/articles/:slug` - 获取文章详情
- `PUT /api/articles/:slug` - 更新文章
- `DELETE /api/articles/:slug` - 删除文章
- `POST /api/articles/:slug/publish` - 发布文章
- `POST /api/articles/:slug/unpublish` - 取消发布
- `GET /api/articles/:slug/versions` - 获取文章版本历史

#### 互动相关
- `POST /api/articles/:slug/clap` - 点赞文章
- `POST /api/articles/:slug/bookmark` - 收藏文章
- `DELETE /api/articles/:slug/bookmark` - 取消收藏
- `GET /api/articles/:slug/comments` - 获取评论
- `POST /api/articles/:slug/comments` - 发表评论
- `PUT /api/comments/:id` - 更新评论
- `DELETE /api/comments/:id` - 删除评论

#### 搜索相关
- `GET /api/search` - 全文搜索
- `GET /api/search/suggestions` - 搜索建议
- `GET /api/tags` - 获取标签列表
- `GET /api/tags/:name/articles` - 获取标签下的文章

#### 推荐相关
- `GET /api/recommendations` - 个性化推荐
- `GET /api/trending` - 热门文章
- `GET /api/articles/:slug/related` - 相关文章

#### 出版物相关
- `GET /api/publications` - 获取出版物列表
- `POST /api/publications` - 创建出版物
- `GET /api/publications/:slug` - 获取出版物详情
- `PUT /api/publications/:slug` - 更新出版物
- `GET /api/publications/:slug/articles` - 获取出版物文章
- `POST /api/publications/:slug/submit` - 投稿到出版物

#### 媒体相关
- `POST /api/media/upload` - 上传图片
- `GET /api/media/:id` - 获取媒体文件

## 开发规范

### 代码规范
1. 遵循 Rust 官方编码规范
2. 使用 `cargo fmt` 格式化代码
3. 使用 `cargo clippy` 进行代码检查
4. 所有公共 API 必须有文档注释

### Git 提交规范
- feat: 新功能
- fix: 修复问题
- docs: 文档修改
- style: 代码格式修改
- refactor: 代码重构
- test: 测试相关
- chore: 其他修改

### 错误处理
1. 使用 `thiserror` 定义错误类型
2. 使用 `anyhow` 处理错误传播
3. 为所有错误提供清晰的错误信息

### 安全性
1. 所有用户输入必须验证
2. 使用参数化查询防止注入
3. 敏感信息必须加密存储
4. 实施速率限制

## 部署架构

### 推荐部署方案
```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Nginx     │────▶│  Rainbow-   │────▶│  SurrealDB  │
│  (Reverse   │     │    Blog     │     │  (Database) │
│   Proxy)    │     │  (Axum App) │     └─────────────┘
└─────────────┘     └─────────────┘              │
                            │                     │
                            ▼                     ▼
                    ┌─────────────┐      ┌─────────────┐
                    │   Redis     │      │   MinIO/S3  │
                    │   (Cache)   │      │  (Storage)  │
                    └─────────────┘      └─────────────┘
```

### 环境变量配置
```env
# 数据库配置
DATABASE_URL=http://localhost:8000
DATABASE_NAMESPACE=rainbow
DATABASE_NAME=blog
DATABASE_USERNAME=root
DATABASE_PASSWORD=root

# JWT 配置
JWT_SECRET=your-secret-key-here
JWT_EXPIRY=7d

# 服务器配置
SERVER_HOST=0.0.0.0
SERVER_PORT=3000

# Redis 配置
REDIS_URL=redis://localhost:6379

# 存储配置
STORAGE_TYPE=s3
S3_BUCKET=rainbow-blog
S3_REGION=us-east-1
S3_ACCESS_KEY=your-access-key
S3_SECRET_KEY=your-secret-key

# 邮件配置
SMTP_HOST=smtp.gmail.com
SMTP_PORT=587
SMTP_USERNAME=your-email@gmail.com
SMTP_PASSWORD=your-password

# 其他配置
LOG_LEVEL=info
ENVIRONMENT=development
```

## 开发计划

### 第一阶段：基础功能（2-3周）
1. 项目初始化和基础架构
2. 用户注册/登录系统
3. 文章的创建、编辑、发布
4. 基础的文章展示和列表

### 第二阶段：核心功能（3-4周）
1. 评论系统
2. 点赞和收藏功能
3. 标签系统
4. 用户关注功能
5. 基础搜索功能

### 第三阶段：高级功能（4-5周）
1. 推荐系统
2. 出版物功能
3. 文章系列
4. 高级搜索和筛选
5. 统计和分析

### 第四阶段：商业功能（3-4周）
1. 会员订阅系统
2. 付费内容
3. 作者收益
4. 广告系统（可选）

### 第五阶段：优化和扩展（持续）
1. 性能优化
2. 缓存策略
3. CDN 集成
4. 国际化
5. 移动端适配

## 测试策略

### 单元测试
- 所有核心业务逻辑必须有单元测试
- 测试覆盖率目标：80%以上

### 集成测试
- API 端点测试
- 数据库操作测试
- 认证流程测试

### 性能测试
- 负载测试
- 压力测试
- 并发测试

## 监控和日志

### 日志级别
- ERROR: 错误信息
- WARN: 警告信息
- INFO: 一般信息
- DEBUG: 调试信息

### 监控指标
- API 响应时间
- 数据库查询性能
- 错误率
- 用户活跃度
- 内容增长率

## 参考资源

1. [Medium 工程博客](https://medium.engineering/)
2. [Axum 官方文档](https://docs.rs/axum/)
3. [SurrealDB 文档](https://surrealdb.com/docs)
4. [Rainbow-docs 源代码](../Rainbow-docs/)
5. [soulcore 文档](../soulcore/)

## 常见问题

### Q: 为什么选择 Rust + Axum？
A: Rust 提供了极高的性能和内存安全性，Axum 是一个现代化的异步 Web 框架，与 Tokio 生态系统完美集成。

### Q: 如何处理并发写入？
A: SurrealDB 内置了 ACID 事务支持，配合 soulcore 的连接池管理，可以有效处理并发场景。

### Q: 如何实现实时功能？
A: 可以使用 WebSocket 或 Server-Sent Events (SSE) 实现实时通知、评论更新等功能。

### Q: 如何优化搜索性能？
A: 使用 SurrealDB 的全文搜索功能，配合适当的索引策略和缓存机制。

## 联系方式

如有问题或建议，请联系 Rainbow Hub 团队。

---

最后更新：2024-01-20