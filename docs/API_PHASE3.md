# Rainbow-Blog Phase 3 API 文档

## 🌈 概述

Rainbow-Blog 第三阶段 API 文档，包含推荐系统、出版物管理、文章系列、高级搜索和统计分析等高级功能。

### 基础信息

- **基础URL**: `http://localhost:3001/api/blog`
- **认证方式**: Bearer Token (JWT)
- **内容类型**: `application/json`
- **字符编码**: UTF-8

### 版本信息

- **API版本**: v1
- **文档更新**: 2024-01-20
- **项目阶段**: 第三阶段开发完成

---

## 🎯 推荐系统 API

推荐系统提供基于内容和协同过滤的文章推荐功能。

### 获取推荐文章

```http
GET /api/blog/recommendations
```

**认证**: 可选（登录用户获得个性化推荐，匿名用户获得热门推荐）

**查询参数**:
- `user_id` (string): 可选，用户ID
- `limit` (integer): 可选，默认10，最大50
- `algorithm` (string): 可选，推荐算法 (`content_based`, `collaborative_filtering`, `hybrid`, `trending`, `following`)
- `exclude_read` (boolean): 可选，默认true，是否排除已读文章
- `tags` (array): 可选，标签过滤
- `authors` (array): 可选，作者过滤

**响应示例**:
```json
{
  "success": true,
  "data": {
    "articles": [
      {
        "article": {
          "id": "article_123",
          "title": "深入理解机器学习",
          "subtitle": "从基础到实践",
          "slug": "deep-understanding-machine-learning",
          "excerpt": "本文将带你深入了解机器学习的核心概念...",
          "cover_image_url": "https://example.com/cover.jpg",
          "author": {
            "id": "user_456",
            "username": "ml_expert",
            "display_name": "机器学习专家",
            "avatar_url": "https://example.com/avatar.jpg",
            "is_verified": true
          },
          "publication": {
            "id": "pub_789",
            "name": "AI技术前沿",
            "slug": "ai-tech-frontier",
            "logo_url": "https://example.com/logo.jpg"
          },
          "status": "published",
          "is_paid_content": false,
          "is_featured": true,
          "reading_time": 8,
          "view_count": 1250,
          "clap_count": 89,
          "comment_count": 12,
          "tags": [
            {
              "id": "tag_001",
              "name": "机器学习",
              "slug": "machine-learning"
            }
          ],
          "created_at": "2023-12-01T10:00:00Z",
          "published_at": "2023-12-01T12:00:00Z"
        },
        "score": 95.5,
        "reason": "基于您对机器学习内容的兴趣"
      }
    ],
    "total": 25,
    "algorithm_used": "Hybrid",
    "generated_at": "2023-12-01T15:30:00Z"
  }
}
```

### 获取热门文章

```http
GET /api/blog/recommendations/trending
```

**认证**: 不需要

**查询参数**:
- `limit` (integer): 可选，默认20，最大100
- `period` (string): 可选，时间范围 (`24h`, `7d`, `30d`)，默认`7d`
- `category` (string): 可选，分类过滤

### 获取关注用户的文章

```http
GET /api/blog/recommendations/following
```

**认证**: 需要

**查询参数**:
- `limit` (integer): 可选，默认20，最大50
- `include_read` (boolean): 可选，默认false，是否包含已读文章

### 获取相关文章

```http
GET /api/blog/recommendations/related/{article_id}
```

**路径参数**:
- `article_id` (string): 文章ID

**认证**: 不需要

**查询参数**:
- `limit` (integer): 可选，默认5，最大20

---

## 🏢 出版物系统 API

出版物系统允许用户创建和管理出版物，支持多级权限管理和协作发布。

### 权限等级

| 角色 | 权限说明 |
|------|----------|
| Owner | 拥有所有权限，包括删除出版物和管理所有成员 |
| Editor | 可以编辑所有文章、管理Writer和Contributor |
| Writer | 可以发布文章到出版物、编辑自己的文章 |
| Contributor | 可以提交文章草稿，需要审核后发布 |

### 创建出版物

```http
POST /api/blog/publications
```

**认证**: 需要

**请求体**:
```json
{
  "name": "AI技术前沿",
  "description": "专注于人工智能和机器学习的最新技术动态",
  "tagline": "探索AI的无限可能",
  "logo_url": "https://example.com/logo.jpg",
  "cover_image_url": "https://example.com/cover.jpg",
  "homepage_layout": "magazine",
  "theme_color": "#2563eb",
  "custom_domain": "ai.example.com"
}
```

**响应示例**:
```json
{
  "success": true,
  "data": {
    "id": "pub_123",
    "name": "AI技术前沿",
    "slug": "ai-tech-frontier",
    "description": "专注于人工智能和机器学习的最新技术动态",
    "tagline": "探索AI的无限可能",
    "logo_url": "https://example.com/logo.jpg",
    "cover_image_url": "https://example.com/cover.jpg",
    "owner_id": "user_456",
    "homepage_layout": "magazine",
    "theme_color": "#2563eb",
    "custom_domain": "ai.example.com",
    "member_count": 1,
    "article_count": 0,
    "follower_count": 0,
    "is_verified": false,
    "is_suspended": false,
    "created_at": "2023-12-01T10:00:00Z",
    "updated_at": "2023-12-01T10:00:00Z"
  },
  "message": "Publication created successfully"
}
```

### 获取出版物详情

```http
GET /api/blog/publications/{slug}
```

**路径参数**:
- `slug` (string): 出版物的slug

**认证**: 可选

### 获取出版物列表

```http
GET /api/blog/publications
```

**认证**: 不需要

**查询参数**:
- `search` (string): 可选，搜索关键词
- `category` (string): 可选，分类过滤
- `sort` (string): 可选，排序方式 (`newest`, `oldest`, `popular`, `alphabetical`)，默认`popular`
- `page` (integer): 可选，页码，默认1
- `limit` (integer): 可选，每页数量，默认20，最大100

### 更新出版物

```http
PUT /api/blog/publications/{slug}
```

**认证**: 需要（Owner或Editor权限）

### 删除出版物

```http
DELETE /api/blog/publications/{slug}
```

**认证**: 需要（Owner权限）

### 添加成员

```http
POST /api/blog/publications/{id}/members
```

**认证**: 需要（Owner或Editor权限）

**请求体**:
```json
{
  "user_id": "user_789",
  "role": "writer",
  "message": "欢迎加入我们的出版物！"
}
```

### 获取成员列表

```http
GET /api/blog/publications/{id}/members
```

**认证**: 需要（成员权限）

**查询参数**:
- `role` (string): 可选，角色过滤 (`owner`, `editor`, `writer`, `contributor`)
- `status` (string): 可选，状态过滤 (`active`, `inactive`)
- `page` (integer): 可选，页码，默认1
- `limit` (integer): 可选，每页数量，默认20

### 更新成员角色

```http
PUT /api/blog/publications/{id}/members/{user_id}
```

**认证**: 需要（Owner或Editor权限）

### 移除成员

```http
DELETE /api/blog/publications/{id}/members/{user_id}
```

**认证**: 需要（Owner权限，或Editor移除Writer/Contributor）

### 关注/取消关注出版物

```http
POST /api/blog/publications/{id}/follow
DELETE /api/blog/publications/{id}/follow
```

**认证**: 需要

### 获取关注的出版物

```http
GET /api/blog/publications/following
```

**认证**: 需要

### 获取出版物文章

```http
GET /api/blog/publications/{slug}/articles
```

**认证**: 可选

**查询参数**:
- `status` (string): 可选，状态过滤 (`published`, `draft`)，默认`published`
- `author` (string): 可选，作者过滤
- `tag` (string): 可选，标签过滤
- `sort` (string): 可选，排序方式 (`newest`, `oldest`, `popular`)，默认`newest`
- `page` (integer): 可选，页码，默认1
- `limit` (integer): 可选，每页数量，默认20

---

## 📚 文章系列 API

文章系列系统允许作者将相关文章组织成系列，支持订阅和顺序管理。

### 创建系列

```http
POST /api/blog/series
```

**认证**: 需要

**请求体**:
```json
{
  "title": "深入理解区块链技术",
  "description": "从基础概念到高级应用的完整系列",
  "cover_image_url": "https://example.com/series-cover.jpg",
  "is_public": true
}
```

**响应示例**:
```json
{
  "success": true,
  "data": {
    "id": "series_123",
    "title": "深入理解区块链技术",
    "slug": "deep-understanding-blockchain",
    "description": "从基础概念到高级应用的完整系列",
    "author_id": "user_456",
    "cover_image_url": "https://example.com/series-cover.jpg",
    "article_count": 0,
    "is_completed": false,
    "is_public": true,
    "view_count": 0,
    "subscriber_count": 0,
    "created_at": "2023-12-01T10:00:00Z",
    "updated_at": "2023-12-01T10:00:00Z"
  },
  "message": "Series created successfully"
}
```

### 获取系列列表

```http
GET /api/blog/series
```

**认证**: 可选（登录用户可看到自己的私有系列）

**查询参数**:
- `author_id` (string): 可选，作者ID过滤
- `is_completed` (boolean): 可选，是否完成过滤
- `is_public` (boolean): 可选，默认true（匿名用户）
- `search` (string): 可选，搜索关键词
- `sort` (string): 可选，排序方式 (`newest`, `oldest`, `popular`, `alphabetical`)，默认`newest`
- `page` (integer): 可选，页码，默认1
- `limit` (integer): 可选，每页数量，默认20，最大100

### 获取系列详情

```http
GET /api/blog/series/{slug}
```

**路径参数**:
- `slug` (string): 系列的slug

**认证**: 可选

**响应示例**:
```json
{
  "success": true,
  "data": {
    "series": {
      "id": "series_123",
      "title": "深入理解区块链技术",
      "slug": "deep-understanding-blockchain",
      "description": "从基础概念到高级应用的完整系列",
      "cover_image_url": "https://example.com/series-cover.jpg",
      "author_id": "user_456",
      "article_count": 8,
      "is_completed": false,
      "is_public": true,
      "view_count": 2540,
      "subscriber_count": 156,
      "created_at": "2023-11-01T10:00:00Z",
      "updated_at": "2023-11-28T14:30:00Z"
    },
    "author_name": "区块链专家",
    "author_username": "blockchain_expert",
    "author_avatar": "https://example.com/avatar.jpg",
    "is_subscribed": false,
    "articles": [
      {
        "id": "article_001",
        "title": "区块链基础概念",
        "subtitle": "什么是区块链？",
        "slug": "blockchain-basic-concepts",
        "excerpt": "区块链是一种分布式账本技术...",
        "cover_image_url": "https://example.com/article1.jpg",
        "reading_time": 5,
        "order_index": 1,
        "is_published": true,
        "published_at": "2023-11-01T12:00:00Z"
      }
    ]
  }
}
```

### 更新系列

```http
PUT /api/blog/series/{slug}
```

**认证**: 需要（系列作者）

### 删除系列

```http
DELETE /api/blog/series/{slug}
```

**认证**: 需要（系列作者）

### 添加文章到系列

```http
POST /api/blog/series/{id}/articles
```

**认证**: 需要（系列作者）

**请求体**:
```json
{
  "article_id": "article_789",
  "order_index": 3
}
```

### 从系列中移除文章

```http
DELETE /api/blog/series/{id}/articles
```

**认证**: 需要（系列作者）

**查询参数**:
- `article_id` (string): 文章ID

### 更新文章顺序

```http
PUT /api/blog/series/{id}/articles/order
```

**认证**: 需要（系列作者）

**请求体**:
```json
{
  "articles": [
    {
      "article_id": "article_001",
      "order_index": 1
    },
    {
      "article_id": "article_002",
      "order_index": 2
    }
  ]
}
```

### 订阅系列

```http
POST /api/blog/series/{id}/subscribe
```

**认证**: 需要

### 取消订阅系列

```http
DELETE /api/blog/series/{id}/subscribe
```

**认证**: 需要

### 获取订阅的系列

```http
GET /api/blog/series/subscribed
```

**认证**: 需要

**查询参数**:
- `page` (integer): 可选，页码，默认1
- `limit` (integer): 可选，每页数量，默认20

---

## 🔍 高级搜索 API

高级搜索系统提供复杂的多维度搜索和筛选功能，支持faceted搜索和智能推荐。

### 基础搜索

```http
GET /api/blog/search
```

**认证**: 可选

**查询参数**:
- `q` (string): 搜索关键词
- `search_type` (string): 可选，搜索类型 (`all`, `articles`, `users`, `tags`, `publications`)，默认`all`
- `page` (integer): 可选，页码，默认1
- `limit` (integer): 可选，每页数量，默认10，最大50

**响应示例**:
```json
{
  "success": true,
  "data": {
    "articles": [
      {
        "id": "article_123",
        "title": "机器学习入门指南",
        "slug": "machine-learning-beginner-guide",
        "excerpt": "本文将介绍机器学习的基本概念...",
        "author_name": "AI专家",
        "author_username": "ai_expert",
        "cover_image_url": "https://example.com/cover.jpg",
        "reading_time": 8,
        "published_at": "2023-11-15T10:00:00Z",
        "clap_count": 156,
        "comment_count": 23,
        "tags": ["机器学习", "AI", "深度学习"],
        "highlight": {
          "field": "title",
          "snippet": "<mark>机器学习</mark>入门指南"
        }
      }
    ],
    "users": [
      {
        "user_id": "user_456",
        "username": "ml_researcher",
        "display_name": "机器学习研究员",
        "avatar_url": "https://example.com/avatar.jpg",
        "bio": "专注于机器学习和深度学习研究",
        "is_verified": true,
        "follower_count": 1250,
        "article_count": 87,
        "highlight": {
          "field": "bio",
          "snippet": "专注于<mark>机器学习</mark>和深度学习研究"
        }
      }
    ],
    "tags": [
      {
        "id": "tag_789",
        "name": "机器学习",
        "slug": "machine-learning",
        "description": "关于机器学习算法和应用的内容",
        "article_count": 234,
        "follower_count": 890,
        "is_featured": true,
        "highlight": {
          "field": "name",
          "snippet": "<mark>机器学习</mark>"
        }
      }
    ],
    "publications": [
      {
        "id": "pub_101",
        "name": "AI与机器学习周刊",
        "slug": "ai-ml-weekly",
        "description": "每周分享最新的AI和机器学习资讯",
        "tagline": "紧跟AI发展步伐",
        "logo_url": "https://example.com/pub-logo.jpg",
        "member_count": 15,
        "article_count": 156,
        "follower_count": 2340,
        "highlight": {
          "field": "name",
          "snippet": "AI与<mark>机器学习</mark>周刊"
        }
      }
    ],
    "total_results": 128
  }
}
```

### 高级搜索

```http
POST /api/blog/search/advanced
```

**认证**: 可选（登录用户可获得个性化结果）

**请求体**:
```json
{
  "q": "深度学习",
  "search_type": "articles",
  "author": "ai_expert",
  "tags": ["深度学习", "神经网络"],
  "publication": "ai-tech-frontier",
  "series": "deep-learning-series",
  "date_from": "2023-01-01T00:00:00Z",
  "date_to": "2023-12-31T23:59:59Z",
  "min_reading_time": 5,
  "max_reading_time": 20,
  "min_claps": 50,
  "is_featured": true,
  "has_audio": false,
  "is_paid": false,
  "sort_by": "relevance",
  "sort_order": "desc",
  "page": 1,
  "limit": 20,
  "include_drafts": false,
  "language": "zh",
  "exclude_read": true
}
```

**响应示例**:
```json
{
  "success": true,
  "data": {
    "articles": [
      {
        "id": "article_456",
        "title": "深度学习的数学基础",
        "slug": "mathematics-foundation-deep-learning",
        "excerpt": "深入理解深度学习背后的数学原理...",
        "author_name": "数学博士",
        "author_username": "math_phd",
        "cover_image_url": "https://example.com/math-dl.jpg",
        "reading_time": 12,
        "published_at": "2023-11-20T14:00:00Z",
        "clap_count": 89,
        "comment_count": 15,
        "tags": ["深度学习", "数学", "神经网络"],
        "highlight": {
          "field": "title",
          "snippet": "<mark>深度学习</mark>的数学基础"
        }
      }
    ],
    "users": [],
    "tags": [],
    "publications": [],
    "series": [
      {
        "id": "series_789",
        "title": "深度学习完整教程",
        "slug": "complete-deep-learning-tutorial",
        "description": "从基础到高级的深度学习系列教程",
        "author_name": "AI导师",
        "author_username": "ai_mentor",
        "article_count": 12,
        "is_completed": true,
        "created_at": "2023-10-01T10:00:00Z",
        "highlight": {
          "field": "title",
          "snippet": "<mark>深度学习</mark>完整教程"
        }
      }
    ],
    "total_results": 45,
    "page": 1,
    "total_pages": 3,
    "facets": {
      "tags": [
        {
          "value": "深度学习",
          "label": "深度学习",
          "count": 156
        },
        {
          "value": "神经网络",
          "label": "神经网络",
          "count": 89
        }
      ],
      "authors": [
        {
          "value": "ai_expert",
          "label": "AI专家",
          "count": 23
        }
      ],
      "publications": [
        {
          "value": "ai-tech-frontier",
          "label": "AI技术前沿",
          "count": 67
        }
      ],
      "date_ranges": [
        {
          "label": "过去24小时",
          "from": "2023-11-30T00:00:00Z",
          "to": "2023-12-01T00:00:00Z",
          "count": 5
        },
        {
          "label": "过去一周",
          "from": "2023-11-24T00:00:00Z",
          "to": "2023-12-01T00:00:00Z",
          "count": 28
        }
      ],
      "reading_time_ranges": [
        {
          "label": "快速阅读（< 3分钟）",
          "min": 0,
          "max": 3,
          "count": 45
        },
        {
          "label": "短文（3-5分钟）",
          "min": 3,
          "max": 5,
          "count": 78
        }
      ]
    }
  }
}
```

### 搜索建议

```http
GET /api/blog/search/suggestions
```

**认证**: 可选

**查询参数**:
- `q` (string): 搜索关键词
- `limit` (integer): 可选，建议数量，默认10，最大20

**响应示例**:
```json
{
  "success": true,
  "data": [
    {
      "text": "机器学习",
      "suggestion_type": "query",
      "metadata": {
        "result_count": 234
      }
    },
    {
      "text": "机器学习算法",
      "suggestion_type": "query",
      "metadata": {
        "result_count": 89
      }
    },
    {
      "text": "机器学习工程师",
      "suggestion_type": "user",
      "metadata": {
        "user_id": "user_123",
        "follower_count": 1250
      }
    }
  ]
}
```

---

## 📊 统计分析 API

统计分析系统为用户和管理员提供详细的数据分析和可视化展示。

### 获取分析仪表板

```http
GET /api/blog/analytics/dashboard
```

**认证**: 需要

**查询参数**:
- `type` (string): 分析类型 (`user`, `publication`)，默认`user`
- `publication_id` (string): 可选，出版物ID（当type=publication时必需）
- `period` (string): 可选，时间范围 (`7d`, `30d`, `90d`, `1y`)，默认`30d`

**响应示例**:
```json
{
  "success": true,
  "data": {
    "overview": {
      "total_views": 15420,
      "total_claps": 892,
      "total_comments": 156,
      "total_followers": 234,
      "views_change": 12.5,
      "claps_change": -3.2,
      "comments_change": 8.7,
      "followers_change": 15.3
    },
    "top_articles": [
      {
        "id": "article_123",
        "title": "深入理解机器学习",
        "slug": "deep-understanding-machine-learning",
        "views": 2340,
        "claps": 156,
        "comments": 23,
        "published_at": "2023-11-15T10:00:00Z"
      }
    ],
    "traffic_sources": [
      {
        "source": "direct",
        "views": 6580,
        "percentage": 42.7
      },
      {
        "source": "search",
        "views": 4320,
        "percentage": 28.0
      },
      {
        "source": "social",
        "views": 2890,
        "percentage": 18.7
      },
      {
        "source": "referral",
        "views": 1630,
        "percentage": 10.6
      }
    ],
    "audience_demographics": {
      "countries": [
        {
          "country": "中国",
          "code": "CN",
          "views": 8450,
          "percentage": 54.8
        }
      ],
      "devices": [
        {
          "device": "desktop",
          "views": 9250,
          "percentage": 60.0
        },
        {
          "device": "mobile",
          "views": 5170,
          "percentage": 33.5
        }
      ]
    },
    "time_range": {
      "start": "2023-11-01T00:00:00Z",
      "end": "2023-12-01T00:00:00Z"
    }
  }
}
```

### 获取用户分析概览

```http
GET /api/blog/analytics/overview
```

**认证**: 需要

**查询参数**:
- `period` (string): 可选，时间范围 (`7d`, `30d`, `90d`, `1y`)，默认`30d`

**响应示例**:
```json
{
  "success": true,
  "data": {
    "user_stats": {
      "total_articles": 45,
      "total_views": 125000,
      "total_claps": 2450,
      "total_comments": 389,
      "total_followers": 1250,
      "total_following": 340,
      "account_age_days": 365,
      "engagement_rate": 3.2
    },
    "growth_metrics": {
      "articles_growth": 5.2,
      "views_growth": 12.8,
      "followers_growth": 18.5,
      "engagement_growth": -2.1
    },
    "content_performance": {
      "avg_views_per_article": 2778,
      "avg_claps_per_article": 54,
      "avg_comments_per_article": 9,
      "most_successful_tag": {
        "name": "机器学习",
        "article_count": 12,
        "avg_views": 3240
      }
    },
    "milestones": [
      {
        "type": "follower_milestone",
        "value": 1000,
        "achieved_at": "2023-11-20T14:30:00Z"
      },
      {
        "type": "view_milestone",
        "value": 100000,
        "achieved_at": "2023-11-25T09:15:00Z"
      }
    ]
  }
}
```

### 获取文章分析详情

```http
GET /api/blog/analytics/articles/{article_id}
```

**路径参数**:
- `article_id` (string): 文章ID

**认证**: 需要（文章作者）

**查询参数**:
- `period` (string): 可选，时间范围 (`7d`, `30d`, `90d`, `1y`)，默认`30d`

### 获取受众分析

```http
GET /api/blog/analytics/audience
```

**认证**: 需要

**查询参数**:
- `publication_id` (string): 可选，出版物ID
- `period` (string): 可选，时间范围 (`7d`, `30d`, `90d`, `1y`)，默认`30d`

### 获取标签分析

```http
GET /api/blog/analytics/tags
```

**认证**: 需要

**查询参数**:
- `period` (string): 可选，时间范围 (`7d`, `30d`, `90d`, `1y`)，默认`30d`
- `limit` (integer): 可选，标签数量，默认20

### 获取趋势分析

```http
GET /api/blog/analytics/trends
```

**认证**: 需要

**查询参数**:
- `metric` (string): 指标类型 (`views`, `claps`, `comments`, `followers`)
- `period` (string): 可选，时间范围 (`7d`, `30d`, `90d`, `1y`)，默认`30d`
- `granularity` (string): 可选，粒度 (`hour`, `day`, `week`, `month`)，默认`day`

### 获取实时分析

```http
GET /api/blog/analytics/realtime
```

**认证**: 需要

**响应示例**:
```json
{
  "success": true,
  "data": {
    "current_active_users": 23,
    "last_updated": "2023-12-01T15:30:00Z",
    "current_hour_views": 145,
    "current_hour_claps": 12,
    "popular_articles_now": [
      {
        "id": "article_456",
        "title": "实时热门文章",
        "current_readers": 8,
        "views_last_hour": 34
      }
    ],
    "traffic_sources_now": [
      {
        "source": "search",
        "active_users": 12
      },
      {
        "source": "direct",
        "active_users": 8
      }
    ],
    "geographic_activity": [
      {
        "country": "中国",
        "active_users": 15
      }
    ]
  }
}
```

### 导出分析数据

```http
POST /api/blog/analytics/export
```

**认证**: 需要

**请求体**:
```json
{
  "type": "overview",
  "format": "csv",
  "period": "30d",
  "filters": {
    "article_ids": ["article_123"],
    "tag_ids": ["tag_456"],
    "publication_id": "pub_789"
  }
}
```

**响应示例**:
```json
{
  "success": true,
  "data": {
    "export_id": "export_123",
    "download_url": "https://example.com/exports/analytics_2023-12-01.csv",
    "expires_at": "2023-12-08T15:30:00Z",
    "file_size": 1024000,
    "record_count": 5420
  }
}
```

---

## 💬 评论系统 API

评论系统支持多层级嵌套回复和点赞功能。

### 获取文章评论

```http
GET /api/blog/comments/{article_id}
```

**路径参数**:
- `article_id` (string): 文章ID

**认证**: 可选

**查询参数**:
- `page` (integer): 可选，页码，默认1
- `limit` (integer): 可选，每页数量，默认20
- `sort` (string): 可选，排序方式 (`newest`, `oldest`, `popular`)，默认`newest`

### 创建评论

```http
POST /api/blog/comments
```

**认证**: 需要

**请求体**:
```json
{
  "article_id": "article_123",
  "content": "这是一条评论内容",
  "parent_id": null
}
```

### 更新评论

```http
PUT /api/blog/comments/{id}
```

**认证**: 需要（评论作者）

### 删除评论

```http
DELETE /api/blog/comments/{id}
```

**认证**: 需要（评论作者）

### 点赞评论

```http
POST /api/blog/comments/{id}/clap
```

**认证**: 需要

### 取消点赞评论

```http
DELETE /api/blog/comments/{id}/clap
```

**认证**: 需要

---

## 👏 点赞系统 API

点赞系统允许用户为文章点赞，每个用户最多可为同一文章点赞50次。

### 为文章点赞

```http
POST /api/blog/articles/{id}/clap
```

**路径参数**:
- `id` (string): 文章ID

**认证**: 需要

**请求体**:
```json
{
  "clap_count": 5
}
```

### 取消文章点赞

```http
DELETE /api/blog/articles/{id}/clap
```

**认证**: 需要

### 获取文章点赞信息

```http
GET /api/blog/articles/{id}/claps
```

**认证**: 可选

---

## 🔖 书签系统 API

书签系统允许用户收藏文章并添加私人笔记。

### 添加书签

```http
POST /api/blog/bookmarks
```

**认证**: 需要

**请求体**:
```json
{
  "article_id": "article_123",
  "notes": "个人笔记内容"
}
```

### 获取用户书签

```http
GET /api/blog/bookmarks
```

**认证**: 需要

**查询参数**:
- `page` (integer): 可选，页码，默认1
- `limit` (integer): 可选，每页数量，默认20
- `sort` (string): 可选，排序方式 (`newest`, `oldest`)，默认`newest`

### 更新书签笔记

```http
PUT /api/blog/bookmarks/{id}
```

**认证**: 需要

### 删除书签

```http
DELETE /api/blog/bookmarks/{id}
```

**认证**: 需要

---

## 🏷️ 标签系统 API

标签系统支持标签管理、关注和文章关联。

### 获取所有标签

```http
GET /api/blog/tags
```

**认证**: 不需要

**查询参数**:
- `page` (integer): 可选，页码，默认1
- `limit` (integer): 可选，每页数量，默认20
- `featured` (boolean): 可选，是否只显示精选标签
- `search` (string): 可选，搜索关键词
- `sort` (string): 可选，排序方式 (`popular`, `alphabetical`, `newest`)，默认`popular`

### 获取标签详情

```http
GET /api/blog/tags/{slug}
```

**路径参数**:
- `slug` (string): 标签的slug

**认证**: 可选

### 获取标签下的文章

```http
GET /api/blog/tags/{slug}/articles
```

**路径参数**:
- `slug` (string): 标签的slug

**认证**: 可选

**查询参数**:
- `page` (integer): 可选，页码，默认1
- `limit` (integer): 可选，每页数量，默认20
- `sort` (string): 可选，排序方式 (`newest`, `popular`)，默认`newest`

### 关注标签

```http
POST /api/blog/tags/{id}/follow
```

**路径参数**:
- `id` (string): 标签ID

**认证**: 需要

### 取消关注标签

```http
DELETE /api/blog/tags/{id}/follow
```

**认证**: 需要

### 获取关注的标签

```http
GET /api/blog/tags/following
```

**认证**: 需要

---

## 👥 关注系统 API

关注系统支持用户之间的关注关系管理。

### 关注用户

```http
POST /api/blog/users/{id}/follow
```

**路径参数**:
- `id` (string): 用户ID

**认证**: 需要

### 取消关注用户

```http
DELETE /api/blog/users/{id}/follow
```

**认证**: 需要

### 获取用户关注列表

```http
GET /api/blog/users/{username}/following
```

**路径参数**:
- `username` (string): 用户名

**认证**: 可选

**查询参数**:
- `page` (integer): 可选，页码，默认1
- `limit` (integer): 可选，每页数量，默认20

### 获取用户粉丝列表

```http
GET /api/blog/users/{username}/followers
```

**路径参数**:
- `username` (string): 用户名

**认证**: 可选

**查询参数**:
- `page` (integer): 可选，页码，默认1
- `limit` (integer): 可选，每页数量，默认20

---

## 🔧 错误处理

### 标准错误响应格式

所有错误响应都遵循以下格式：

```json
{
  "error": {
    "code": "ERROR_CODE",
    "message": "人类可读的错误描述"
  }
}
```

### 常见错误码

| 状态码 | 错误码 | 描述 |
|--------|--------|------|
| 400 | `VALIDATION_ERROR` | 请求数据验证失败 |
| 400 | `BAD_REQUEST` | 请求格式错误 |
| 401 | `AUTHENTICATION_ERROR` | 未认证或Token无效 |
| 403 | `AUTHORIZATION_ERROR` | 权限不足 |
| 404 | `NOT_FOUND` | 资源不存在 |
| 409 | `CONFLICT` | 资源冲突 |
| 429 | `RATE_LIMIT_EXCEEDED` | 请求频率超限 |
| 500 | `INTERNAL_ERROR` | 服务器内部错误 |

### 第三阶段特定错误

#### 推荐系统错误
- `INVALID_ALGORITHM`: 不支持的推荐算法
- `INSUFFICIENT_USER_DATA`: 用户数据不足，无法生成个性化推荐

#### 出版物系统错误
- `PUBLICATION_NOT_FOUND`: 出版物不存在
- `INSUFFICIENT_PERMISSIONS`: 权限不足，无法执行操作
- `MEMBER_ALREADY_EXISTS`: 成员已存在
- `MEMBER_LIMIT_REACHED`: 成员数量已达上限

#### 系列系统错误
- `SERIES_NOT_FOUND`: 系列不存在
- `ARTICLE_ALREADY_IN_SERIES`: 文章已在系列中
- `SERIES_ALREADY_SUBSCRIBED`: 已经订阅了该系列

#### 搜索系统错误
- `INVALID_SEARCH_QUERY`: 搜索查询无效
- `SEARCH_RATE_LIMIT_EXCEEDED`: 搜索频率超限
- `INVALID_SORT_PARAMETER`: 无效的排序参数

#### 分析系统错误
- `INSUFFICIENT_DATA`: 数据不足，无法生成分析
- `INVALID_METRIC`: 无效的指标类型
- `EXPORT_FAILED`: 数据导出失败

---

## 📊 使用限制

### 推荐系统限制
- 匿名用户：每分钟最多30次请求
- 登录用户：每分钟最多60次请求
- 推荐结果会缓存5分钟

### 出版物系统限制
- 每个用户最多可创建5个出版物
- 每个出版物最多可有100个成员
- 成员邀请有7天过期时间

### 系列系统限制
- 每个用户最多可创建20个系列
- 每个系列最多可包含50篇文章
- 系列标题长度限制为200字符

### 搜索系统限制
- 搜索词最小长度：2个字符
- 搜索词最大长度：100个字符
- 每页最多返回50个结果
- 匿名用户每分钟最多20次搜索
- 登录用户每分钟最多60次搜索

### 分析系统限制
- 免费用户：最多查看30天的数据
- 高级用户：可查看1年的历史数据
- 数据导出：每天最多5次
- 实时数据：每分钟最多10次请求
- 分析API：每小时最多100次请求

---

## 🚀 使用示例

### JavaScript 示例

```javascript
// 获取推荐文章
async function getRecommendations(token, limit = 10) {
  const response = await fetch(
    `http://localhost:3001/api/blog/recommendations?limit=${limit}`,
    {
      headers: token ? { 'Authorization': `Bearer ${token}` } : {}
    }
  );
  return response.json();
}

// 创建出版物
async function createPublication(publicationData, token) {
  const response = await fetch('http://localhost:3001/api/blog/publications', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${token}`
    },
    body: JSON.stringify(publicationData)
  });
  return response.json();
}

// 高级搜索
async function advancedSearch(searchData) {
  const response = await fetch('http://localhost:3001/api/blog/search/advanced', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json'
    },
    body: JSON.stringify(searchData)
  });
  return response.json();
}

// 获取分析数据
async function getAnalytics(token, period = '30d') {
  const response = await fetch(
    `http://localhost:3001/api/blog/analytics/dashboard?period=${period}`,
    {
      headers: {
        'Authorization': `Bearer ${token}`
      }
    }
  );
  return response.json();
}
```

### Python 示例

```python
import requests

BASE_URL = "http://localhost:3001/api/blog"

class RainbowBlogClient:
    def __init__(self, token=None):
        self.token = token
        self.headers = {
            'Content-Type': 'application/json'
        }
        if token:
            self.headers['Authorization'] = f'Bearer {token}'
    
    def get_recommendations(self, limit=10, algorithm='hybrid'):
        response = requests.get(
            f"{BASE_URL}/recommendations",
            params={'limit': limit, 'algorithm': algorithm},
            headers=self.headers
        )
        return response.json()
    
    def create_publication(self, publication_data):
        response = requests.post(
            f"{BASE_URL}/publications",
            json=publication_data,
            headers=self.headers
        )
        return response.json()
    
    def advanced_search(self, search_query):
        response = requests.post(
            f"{BASE_URL}/search/advanced",
            json=search_query,
            headers=self.headers
        )
        return response.json()
    
    def get_analytics_dashboard(self, period='30d'):
        response = requests.get(
            f"{BASE_URL}/analytics/dashboard",
            params={'period': period},
            headers=self.headers
        )
        return response.json()

# 使用示例
client = RainbowBlogClient(token="your-jwt-token")

# 获取推荐文章
recommendations = client.get_recommendations(limit=20)

# 创建出版物
publication = client.create_publication({
    "name": "技术周刊",
    "description": "分享最新技术动态",
    "tagline": "技术改变世界"
})

# 高级搜索
results = client.advanced_search({
    "q": "机器学习",
    "tags": ["AI", "机器学习"],
    "min_reading_time": 5,
    "sort_by": "relevance"
})

# 获取分析数据
analytics = client.get_analytics_dashboard(period='30d')
```

---

## 📝 更新日志

### v3.0.0 (2024-01-20) - 第三阶段完成

**新增功能**:
- ✅ 智能推荐系统（内容推荐、协同过滤、混合算法）
- ✅ 出版物管理系统（多级权限、协作发布）
- ✅ 文章系列系统（有序组织、订阅功能）
- ✅ 高级搜索系统（多维搜索、faceted结果）
- ✅ 统计分析系统（实时分析、数据导出）
- ✅ 完整的评论系统（嵌套回复、点赞）
- ✅ 点赞系统（多次点赞支持）
- ✅ 书签系统（私人笔记）
- ✅ 标签管理（关注、分类）
- ✅ 用户关注系统

**API端点**:
- 推荐系统：4个端点
- 出版物系统：12个端点
- 系列系统：11个端点
- 高级搜索：3个端点
- 统计分析：8个端点
- 评论系统：6个端点
- 点赞系统：3个端点
- 书签系统：4个端点
- 标签系统：6个端点
- 关注系统：4个端点

**技术改进**:
- 智能推荐算法实现
- 复杂权限管理系统
- 全文搜索和faceted搜索
- 实时数据分析
- 数据导出功能

---

## 📞 支持与反馈

如有问题或建议，请联系 Rainbow Hub 开发团队。

**项目仓库**: Rainbow-Hub/Rainbow-Blog  
**文档更新**: 2024-01-20  
**维护状态**: ✅ 积极维护中

---

*本文档基于 Rainbow-Blog v3.0.0 生成，涵盖第三阶段所有高级功能的 API 端点。*