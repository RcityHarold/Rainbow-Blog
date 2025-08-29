# Rainbow-Blog API 文档 - 第二阶段功能

## 概述

第二阶段实现了完整的社交互动功能，包括评论系统、点赞（拍手）、书签收藏、标签管理、用户关注和搜索功能。

## API 端点

### 1. 评论系统 (Comments)

#### 获取文章评论
```
GET /api/blog/comments/article/:article_id
```
返回嵌套的评论树结构，包含作者信息和回复。

#### 创建评论
```
POST /api/blog/comments
Authorization: Bearer {token}

{
  "article_id": "文章ID",
  "parent_id": "父评论ID（可选）",
  "content": "评论内容"
}
```

#### 更新评论
```
PUT /api/blog/comments/:id
Authorization: Bearer {token}

{
  "content": "更新的评论内容"
}
```

#### 删除评论
```
DELETE /api/blog/comments/:id
Authorization: Bearer {token}
```

#### 为评论点赞
```
POST /api/blog/comments/:id/clap
Authorization: Bearer {token}
```

#### 取消评论点赞
```
DELETE /api/blog/comments/:id/clap
Authorization: Bearer {token}
```

### 2. 文章点赞系统 (Claps)

#### 为文章点赞
```
POST /api/blog/articles/:id/clap
Authorization: Bearer {token}

{
  "article_id": "文章ID",
  "count": 5  // 1-50
}
```

响应：
```json
{
  "success": true,
  "data": {
    "user_clap_count": 15,
    "total_claps": 1250
  }
}
```

### 3. 书签系统 (Bookmarks)

#### 获取用户书签列表
```
GET /api/blog/bookmarks?page=1&limit=20
Authorization: Bearer {token}
```

#### 添加书签
```
POST /api/blog/bookmarks
Authorization: Bearer {token}

{
  "article_id": "文章ID",
  "note": "私人笔记（可选）"
}
```

#### 更新书签笔记
```
PUT /api/blog/bookmarks/:id
Authorization: Bearer {token}

{
  "note": "更新的笔记"
}
```

#### 删除书签
```
DELETE /api/blog/bookmarks/:id
Authorization: Bearer {token}
```

#### 通过文章ID删除书签
```
DELETE /api/blog/bookmarks/article/:article_id
Authorization: Bearer {token}
```

#### 检查文章是否已收藏
```
GET /api/blog/bookmarks/check/:article_id
Authorization: Bearer {token}
```

### 4. 标签系统 (Tags)

#### 获取标签列表
```
GET /api/blog/tags?search=keyword&featured_only=true&sort_by=popular&page=1&limit=20
```

#### 创建标签（管理员）
```
POST /api/blog/tags
Authorization: Bearer {token}

{
  "name": "标签名称",
  "description": "标签描述"
}
```

#### 更新标签（管理员）
```
PUT /api/blog/tags/:id
Authorization: Bearer {token}

{
  "name": "新名称",
  "description": "新描述",
  "is_featured": true
}
```

#### 删除标签（管理员）
```
DELETE /api/blog/tags/:id
Authorization: Bearer {token}
```

#### 获取标签详情
```
GET /api/blog/tags/slug/:slug
```

#### 获取文章的标签
```
GET /api/blog/tags/article/:article_id
```

#### 为文章添加标签
```
POST /api/blog/tags/article/:article_id/tags
Authorization: Bearer {token}

["tag_id_1", "tag_id_2"]
```

#### 移除文章标签
```
DELETE /api/blog/tags/article/:article_id/tags
Authorization: Bearer {token}

["tag_id_1", "tag_id_2"]
```

#### 关注标签
```
POST /api/blog/tags/:id/follow
Authorization: Bearer {token}
```

#### 取消关注标签
```
DELETE /api/blog/tags/:id/follow
Authorization: Bearer {token}
```

#### 获取已关注的标签
```
GET /api/blog/tags/followed
Authorization: Bearer {token}
```

### 5. 用户关注系统 (Follows)

#### 关注用户
```
POST /api/blog/follows/user/:user_id/follow
Authorization: Bearer {token}
```

#### 取消关注用户
```
DELETE /api/blog/follows/user/:user_id/follow
Authorization: Bearer {token}
```

#### 获取用户的关注者
```
GET /api/blog/follows/user/:user_id/followers?page=1&limit=20
```

#### 获取用户关注的人
```
GET /api/blog/follows/user/:user_id/following?page=1&limit=20
```

#### 获取关注统计
```
GET /api/blog/follows/user/:user_id/stats
```

响应：
```json
{
  "success": true,
  "data": {
    "followers_count": 1250,
    "following_count": 89,
    "is_following": true,
    "is_followed_by": false
  }
}
```

#### 检查是否关注某用户
```
GET /api/blog/follows/user/:user_id/is-following
Authorization: Bearer {token}
```

#### 获取共同关注
```
GET /api/blog/follows/mutual/:target_user_id?limit=10
Authorization: Bearer {token}
```

### 6. 搜索系统 (Search)

#### 全局搜索
```
GET /api/blog/search?q=关键词&search_type=all&page=1&limit=10
```

搜索类型：
- `all` - 搜索所有内容
- `articles` - 仅搜索文章
- `users` - 仅搜索用户
- `tags` - 仅搜索标签
- `publications` - 仅搜索出版物

响应：
```json
{
  "success": true,
  "data": {
    "articles": [...],
    "users": [...],
    "tags": [...],
    "publications": [...],
    "total_results": 125
  }
}
```

#### 搜索建议
```
GET /api/blog/search/suggestions?q=关键词&limit=10
```

响应：
```json
{
  "success": true,
  "data": [
    {
      "text": "rust programming",
      "suggestion_type": "query",
      "metadata": null
    },
    {
      "text": "Rust",
      "suggestion_type": "tag",
      "metadata": {
        "slug": "rust",
        "article_count": 125
      }
    }
  ]
}
```

## 数据模型

### Comment (评论)
- 支持嵌套回复
- 作者回复特殊标记
- 软删除支持
- 点赞功能

### Clap (点赞)
- 每个用户每篇文章最多50次
- 实时更新文章总点赞数

### Bookmark (书签)
- 支持私人笔记
- 快速收藏/取消收藏

### Follow (关注)
- 用户间关注关系
- 实时更新关注者/关注数
- 关注通知

### Tag (标签)
- 支持关注标签
- 文章标签关联
- 热门标签推荐

### Search (搜索)
- 全文搜索
- 高亮显示
- 搜索建议
- 按相关度排序

## 特性亮点

1. **嵌套评论系统**：支持多级回复，自动标记作者回复
2. **Medium风格点赞**：支持多次点赞，最多50次
3. **智能搜索**：支持全文搜索、高亮显示和搜索建议
4. **社交关注**：完整的用户关注系统，包括共同关注推荐
5. **标签系统**：支持标签关注和热门标签推荐
6. **书签收藏**：支持私人笔记的书签系统

## 权限说明

- 评论：需要登录
- 点赞：需要登录
- 书签：需要登录
- 关注：需要登录
- 标签管理：需要管理员权限
- 搜索：公开访问

## 通知集成

以下操作会触发通知：
- 用户被关注
- 文章收到评论
- 评论收到回复
- 评论被点赞

## 性能优化

- 搜索索引自动更新
- 统计数据异步更新
- 批量查询优化
- 缓存热门内容