# API Reference — Sowling Backend

Base URL: `http://localhost:8080`

All authenticated endpoints require: `Authorization: Bearer <access_token>`

---

## Health

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | /health | No | Health check |

---

## Authentication

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | /auth/register | No | Register new account |
| POST | /auth/login | No | Login with email/password |
| POST | /auth/google | No | Google OAuth (not yet implemented) |
| POST | /auth/logout | Yes | Logout (revoke refresh token) |
| POST | /auth/refresh | No | Refresh access token |

### POST /auth/register
```json
Request:  { "email": "user@example.com", "password": "min8chars" }
Response: { "user_id": "uuid", "auth_mode": "email", "access_token": "jwt", "refresh_token": "hex" }
```

### POST /auth/login
```json
Request:  { "email": "user@example.com", "password": "password" }
Response: { "user_id": "uuid", "auth_mode": "email", "access_token": "jwt", "refresh_token": "hex" }
```

### POST /auth/refresh
```json
Request:  { "refresh_token": "hex" }
Response: { "user_id": "uuid", "auth_mode": "email", "access_token": "new_jwt", "refresh_token": "new_hex" }
```

### POST /auth/logout
```json
Request:  { "refresh_token": "hex" }  (optional)
Response: { "status": "ok" }
```

Rate limited: 10 requests per 60 seconds per IP.

---

## User Profile

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | /me | Yes | Get current user profile |
| PATCH | /me | Yes | Update profile (locale, timezone) |
| DELETE | /me | Yes | Delete account (GDPR cascade) |
| PATCH | /me/adult-confirmation | Yes | Confirm adult status |
| POST | /me/change-password | Yes | Change password |

### PATCH /me
```json
Request:  { "locale": "de-DE", "timezone": "Europe/Berlin" }
Response: { "status": "ok" }
```

### POST /me/change-password
```json
Request:  { "current_password": "old", "new_password": "new_min8" }
Response: { "status": "ok" }
```

---

## Plants

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | /plants | Yes | List all plants |
| POST | /plants | Yes | Create plant |
| GET | /plants/{id} | Yes | Get single plant |
| PATCH | /plants/{id} | Yes | Update plant |
| DELETE | /plants/{id} | Yes | Delete plant (soft) |

### POST /plants
```json
Request: {
  "name": "Tomate #1",
  "color_tag": "#e74c3c",
  "phase": "seed"
}
Response: Plant object
```

---

## Plant Seasons (Perennials)

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | /plants/{id}/seasons | Yes | List seasons for plant |
| POST | /plants/{id}/seasons | Yes | Create new season |
| PATCH | /seasons/{id} | Yes | Update season |
| DELETE | /seasons/{id} | Yes | Delete season |

### POST /plants/{id}/seasons
```json
Request: { "year": 2026, "notes": "Guter Start" }
Response: PlantSeason object
```

---

## Tasks

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | /plants/{id}/tasks | Yes | List tasks for plant |
| POST | /plants/{id}/tasks | Yes | Create task |
| PATCH | /tasks/{id} | Yes | Update task |
| DELETE | /tasks/{id} | Yes | Delete task |

### POST /plants/{id}/tasks
```json
Request: {
  "title": "Giessen",
  "category": "water",
  "due_at": "2026-04-15T09:00:00Z",
  "repeat_interval_hours": 48,
  "notification_enabled": true
}
Response: Task object
```

---

## Logs

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | /plants/{id}/logs | Yes | List logs for plant |
| POST | /plants/{id}/logs | Yes | Create log |
| PATCH | /logs/{id} | Yes | Update log |
| DELETE | /logs/{id} | Yes | Delete log |

### POST /plants/{id}/logs
```json
Request: {
  "log_type": "watering",
  "text": "Gegossen",
  "created_at": "2026-04-12T10:00:00Z"
}
Response: LogEntry object
```

---

## Locations

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | /locations | Yes | List locations |
| POST | /locations | Yes | Create location |
| PATCH | /locations/{id} | Yes | Update location |
| DELETE | /locations/{id} | Yes | Delete location |

### POST /locations
```json
Request: {
  "name": "Growzelt 1",
  "location_type": "growzelt",
  "description": "2x2m, LED"
}
Response: Location object
```

---

## Catalog

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | /catalog | No | Get full plant catalog (34 entries) |

Returns array of CatalogEntry objects with: name, emoji, category, sow/harvest months, companion planting, toxicity data, propagation methods.

---

## Subscription

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | /subscription | Yes | Get current subscription |
| GET | /subscription/status | Yes | Get subscription status summary |
| GET | /subscription/history | Yes | Get subscription history |
| POST | /subscription/checkout | Yes | Create Stripe checkout session |
| GET | /subscription/checkout/return | No | Checkout return URL |
| POST | /subscription/webhook | No | Stripe webhook endpoint |
| POST | /subscription/portal | Yes | Create Stripe customer portal |

### POST /subscription/checkout
```json
Request: { "plan": "basic" }
Response: { "checkout_url": "https://checkout.stripe.com/..." }
```

Webhook validates Stripe-Signature header. Events handled: checkout.session.completed, customer.subscription.deleted.

---

## Sync

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | /sync/push | Yes | Push local changes to server |
| GET | /sync/pull | Yes | Pull server changes |

### POST /sync/push
Accepts SyncPushRequest with envelope (plants, tasks, logs) + deleted records.
Max 500 records per push. Wrapped in PostgreSQL transaction.
Uses Last-Write-Wins (WHERE updated_at < EXCLUDED).

Ownership verified: plants must belong to user, tasks/logs must belong to user's plants.

---

## Error Responses

All errors return JSON:
```json
{
  "error": "description"
}
```

HTTP Status Codes:
- 200: Success
- 201: Created
- 400: Bad Request (validation)
- 401: Unauthorized
- 403: Forbidden (ownership)
- 413: Payload Too Large (sync)
- 429: Too Many Requests (rate limit)
- 500: Internal Server Error
- 501: Not Implemented (Google OAuth)
