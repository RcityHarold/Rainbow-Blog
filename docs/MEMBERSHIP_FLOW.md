# Rainbow Blog Membership & Stripe Integration Blueprint

_Last updated: 2025-09-18_

This document defines the end-to-end flow for Rainbow Blog's membership system, covering subscription plans, premium content gating, payment collection via Stripe, and revenue reporting. It serves as the contract between backend and frontend teams and enumerates the required API surface.

## 1. Domain Overview

| Actor | Responsibilities |
|-------|------------------|
| **Creator** | Creates subscription plans, configures paid articles, connects Stripe account, views revenue. |
| **Subscriber** | Saves payment method, purchases subscription or single article, consumes premium content, manages billing. |
| **Platform** | Hosts content, orchestrates Stripe payments, tracks access, computes revenue, sends lifecycle notifications. |

### Core Entities

- **SubscriptionPlan** – Creator-owned plan (price, currency, benefits).
- **PaymentMethod** – Stripe payment method stored against subscriber/customer.
- **Subscription** – Active billing relationship (Stripe subscription + internal record).
- **ArticlePricing** – Per-article pricing/premium configuration.
- **ContentAccess** – Access decision cached per request.
- **CreatorRevenue** – Aggregated payouts (subscription + one-time sales).

## 2. Stripe Account & Customer Lifecycle

1. **Creator onboarding**
   - Creator requests `/api/blog/stripe/connect/accounts` → backend creates Stripe Connect account & returns onboarding URL.
   - Creator completes onboarding via Stripe hosted page.
   - Backend stores `stripe_account_id` on user profile.

2. **Subscriber onboarding**
   - Subscriber calls `/api/blog/stripe/customers` (auto-created on first payment interaction) → backend ensures Stripe customer & returns `client_secret` for SetupIntent when necessary.
   - Subscriber adds card via `/api/blog/payment-methods` (see §4.1) which internally creates/attaches PaymentMethod to Stripe customer using Stripe.js + SetupIntent.

## 3. End-to-End Flows

### 3.1 Plan Creation & Management (Creator)

1. Creator navigates Settings → Plans (`/settings#plans`).
2. Frontend fetches `GET /api/blog/subscriptions/creator/{creator_id}/plans`.
3. Creator submits plan via `POST /api/blog/subscriptions/plans` (auth required).
4. Backend persists plan, creates Stripe product & price, returns plan object.
5. Plan updates / deactivation handled via `PUT /api/blog/subscriptions/plans/:id` and `DELETE /api/blog/subscriptions/plans/:id`.

### 3.2 Payment Method Management (Subscriber)

1. Subscriber opens billing settings (`/settings#billing`).
2. Frontend calls `POST /api/blog/stripe/payment-intents` with payload `{ "mode": "setup" }` to obtain a SetupIntent `client_secret`.
3. Stripe Elements collects card data and confirms the SetupIntent.
4. On success, frontend posts to `POST /api/blog/payment-methods` with `{ "payment_method_id": "pm_xxx", "set_as_default": true }`.
5. Backend attaches card to Stripe customer, stores metadata, returns PaymentMethod summary.
6. Subscriber can manage methods via `GET /api/blog/payment-methods`, `POST /api/blog/payment-methods/{id}/default`, `DELETE /api/blog/payment-methods/{id}`.

### 3.3 Subscribe to a Plan (Subscriber)

1. Subscriber opens premium content → `SubscriptionWidget` loads creator plans + current status.
2. User selects a plan; frontend ensures a default payment method, otherwise prompts to add one.
3. Frontend calls `POST /api/blog/subscriptions` with `{ "plan_id": "plan_xxx" }` (optionally `payment_method_id`).
4. Backend:
   - Validates plan & payment method.
   - Creates Stripe subscription (trial, discounts, Connect account destination charges).
   - Persists internal subscription (including `stripe_subscription_id`).
   - Returns subscription details & access rights.
5. Frontend updates UI and unlocks premium content.

### 3.4 Access Premium Content

1. Article page requests `GET /api/blog/payments/content/{article_id}/access`.
2. PaymentService checks: article free?, requester author?, active subscription?, single purchase?
3. Response includes `has_access`, `access_type`, optional `subscription_id`. If access denied, frontend loads preview via `GET /api/blog/payments/content/{article_id}/preview`.

### 3.5 Billing Webhooks & Renewal

1. Stripe sends events to `/api/blog/subscriptions/webhook/stripe` and `/api/blog/stripe/webhooks`.
2. Backend processes `invoice.payment_succeeded`, `invoice.payment_failed`, `customer.subscription.updated`, etc., updating subscription status and notifying users.
3. Aggregated revenue feeds creator dashboard and payout logic.

### 3.6 Creator Revenue Dashboard

1. Creator opens `/earnings` page.
2. Frontend calls `GET /api/blog/subscriptions/creator/{creator_id}/earnings?period=month`.
3. Backend returns metrics: total earnings, active subs, churn, plan breakdown, payout status.
4. Additional analytics via `GET /api/blog/payments/dashboard/{creator_id}` complement subscription revenue with one-time purchases.

## 4. API Contract Summary

### 4.1 Payment Method APIs (required)

| Method | Path | Auth | Notes |
|--------|------|------|-------|
| `GET` | `/api/blog/payment-methods` | Subscriber | List PaymentMethod summaries. |
| `POST` | `/api/blog/payment-methods` | Subscriber | Body `{ payment_method_id, set_as_default }`. Attaches Stripe PM. |
| `DELETE` | `/api/blog/payment-methods/{id}` | Subscriber | Removes PM (if not default). |
| `POST` | `/api/blog/payment-methods/{id}/default` | Subscriber | Sets default PM. |
| `POST` | `/api/blog/stripe/payment-intents` | Subscriber | Request SetupIntent / PaymentIntent; body `{ mode: "setup"|"payment", amount?, currency?, metadata }`. Returns `client_secret`. |

### 4.2 Subscription APIs (alignments needed)

- Replace placeholder `auth_user_id = "user_123"` with authenticated user extraction.
- Ensure `POST /api/blog/subscriptions` demands a payment method when subscriber lacks default.
- `GET /api/blog/subscriptions/creator/{id}/status` should return 404 for missing subscription rather than swallowing errors.
- Implement filtering/pagination in `GET /api/blog/subscriptions/user/{id}`.
- Handle Stripe webhook events to sync subscription lifecycle.

### 4.3 Stripe Service APIs

- Implement `get_customer`, `get_payment_intent`, `confirm_payment_intent`, `get_subscription` or remove unused endpoints.
- Provide Connect onboarding endpoint returning `account_link_url` and persist `stripe_account_id`.

## 5. Frontend Responsibilities

### Stripe Client Integration

- Add Stripe.js (e.g. `@stripe/stripe-js`) and expose bindings to Dioxus for Stripe Elements rendering.
- Build a reusable card entry component to handle SetupIntent confirmation.
- Manage publishable key via environment (`STRIPE_PUBLISHABLE_KEY`) or config endpoint.

### UI Surfaces

- **Settings → Billing**: CRUD payment methods, default selection, show active subscriptions.
- **Subscription Widget**: Plan selection, add-card prompt, subscription status indicator, cancel action.
- **Earnings Dashboard**: Charts fed by `/api/blog/subscriptions/creator/{creator_id}/earnings`.
- **Article Editor**: Paid content toggle & preview percentage wired to `/api/blog/payments/articles/:id/pricing`.

## 6. Backend TODO Checklist

- [x] Replace all `user_123` placeholders with authenticated user IDs in subscription & stripe routes.
- [ ] Implement payment-method endpoints in `payments.rs` leveraging Stripe service.
- [ ] Complete Stripe service functions: customers, SetupIntents, subscriptions, cancelation, Connect onboarding.
- [ ] Verify webhook signature handling and update subscription/payment status accordingly.
- [ ] Add migrations/columns for `stripe_customer_id`, `stripe_account_id`, payment method metadata if missing.

## 7. Frontend TODO Checklist

- [ ] Integrate Stripe.js and publishable key handling in the Dioxus app.
- [ ] Implement payment method management UI and wire to new APIs.
- [ ] Update `SubscriptionWidget` to handle payment method requirements, loading, and errors.
- [ ] Provide subscription cancellation and billing history surfaces.
- [ ] Expand settings navigation with billing/Stripe tabs and connect status display.
- [ ] Surface premium gating prompts linking to subscription/billing flows.

## 8. Testing Strategy

- Unit tests for subscription/payment services, webhook handlers, and access checks.
- Integration tests simulating plan creation → payment method addition → subscription purchase.
- Manual QA in Stripe test mode (4242 series), including Connect onboarding with test accounts.

## 9. Rollout Considerations

- Feature flag membership UI until backend endpoints are production-ready.
- Ship DB migrations carefully (Stripe columns + payment method storage).
- Configure webhook secrets per environment and secure Stripe API key handling.

---

This blueprint will be updated as implementation progresses. Track checklist items within this document to reflect delivery status.
