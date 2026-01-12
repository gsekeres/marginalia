# Deploying Marginalia

This guide covers deploying Marginalia with:
- **Frontend**: Static HTML on your Hugo site (or any static host)
- **Backend**: FastAPI on Render (free tier works for personal use)

## Overview

```
┌─────────────────────────────────────┐
│     Your Hugo Site (Static)         │
│  gabesekeres.com/marginalia/          │
│  └── dashboard.html                 │
└─────────────────────────────────────┘
              │
              │ API calls
              ▼
┌─────────────────────────────────────┐
│     Render (Python Backend)          │
│  marginalia-api.onrender.com          │
│  ├── FastAPI server                  │
│  ├── Claude Code CLI (Node.js)      │
│  └── Persistent disk (vault data)   │
└─────────────────────────────────────┘
```

---

## Step 1: Prepare the Backend for Render

### 1.1 Create `render.yaml`

Create a `render.yaml` in your repo root:

```yaml
services:
  - type: web
    name: marginalia-api
    runtime: python
    buildCommand: |
      pip install -e .
      curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
      apt-get install -y nodejs
      npm install -g @anthropic-ai/claude-code
    startCommand: uvicorn agents.api:app --host 0.0.0.0 --port $PORT
    envVars:
      - key: CLAUDE_CODE_OAUTH_TOKEN
        sync: false
      - key: UNPAYWALL_EMAIL
        sync: false
      - key: VAULT_PATH
        value: /var/data/vault
    disk:
      name: vault-data
      mountPath: /var/data
      sizeGB: 1
```

### 1.2 Create `Procfile` (alternative to render.yaml)

```
web: uvicorn agents.api:app --host 0.0.0.0 --port $PORT
```

### 1.3 Create `runtime.txt`

```
python-3.12.0
```

---

## Step 2: Deploy to Render

### 2.1 Create a New Web Service

1. Go to [render.com](https://render.com) and sign up/log in
2. Click **New** → **Web Service**
3. Connect your GitHub repo (or use "Public Git repository")
4. Configure:
   - **Name**: `marginalia-api`
   - **Region**: Choose closest to you
   - **Branch**: `main`
   - **Runtime**: Python 3
   - **Build Command**:
     ```
     pip install -e . && curl -fsSL https://deb.nodesource.com/setup_20.x | bash - && apt-get install -y nodejs && npm install -g @anthropic-ai/claude-code
     ```
   - **Start Command**: `uvicorn agents.api:app --host 0.0.0.0 --port $PORT`

### 2.2 Add Persistent Disk

1. In service settings, go to **Disks**
2. Add disk:
   - **Name**: `vault-data`
   - **Mount Path**: `/var/data`
   - **Size**: 1 GB (adjust as needed)

### 2.3 Add Environment Variables

Go to **Environment** and add:

| Key | Value |
|-----|-------|
| `CLAUDE_CODE_OAUTH_TOKEN` | Your token from `claude setup-token` |
| `UNPAYWALL_EMAIL` | Your email (optional, improves rate limits) |
| `VAULT_PATH` | `/var/data/vault` |

**Important**: Do NOT add `ANTHROPIC_API_KEY` - it will break OAuth authentication.

### 2.4 Deploy

Click **Create Web Service**. Render will build and deploy automatically.

Your API will be available at: `https://marginalia-api.onrender.com`

---

## Step 3: Deploy Frontend to Hugo Site

### 3.1 Copy Dashboard to Hugo Static Files

Copy the dashboard HTML to your Hugo site:

```bash
# From Marginalia repo
cp app/index.html /path/to/mysite-hugo/static/marginalia/dashboard.html
```

### 3.2 Update API URL in Dashboard

Edit `/static/marginalia/dashboard.html` and update the API URL:

```javascript
// Configure API endpoint for Render deployment
window.MARGINALIA_API_BASE = 'https://marginalia-api.onrender.com';
```

### 3.3 Create Landing Page (Optional)

Create `/content/marginalia/_index.md`:

```markdown
---
title: "Marginalia"
description: "Agent-based academic literature management"
---

# Marginalia

An agent-based tool for managing academic literature.

[**Open Dashboard**](/marginalia/dashboard.html)

---

## How It Works

1. Import your BibTeX bibliography
2. Mark papers you want to read
3. Marginalia searches for open-access PDFs
4. Summarize papers with Claude

## Open Source

Available on [GitHub](https://github.com/gsekeres/marginalia).
```

### 3.4 Deploy Hugo Site

```bash
cd /path/to/mysite-hugo
hugo
# Deploy public/ to your host (Netlify, GitHub Pages, etc.)
```

---

## Step 4: Verify Deployment

### 4.1 Test API

```bash
curl https://marginalia-api.onrender.com/api/stats
```

Should return:
```json
{"total": 0, "by_status": {"discovered": 0, ...}}
```

### 4.2 Test Dashboard

1. Open `https://yourdomain.com/marginalia/dashboard.html`
2. You should see the dashboard with "API Not Connected" if empty, or stats if you have data

### 4.3 Test Summarization

1. Import a BibTeX file through the dashboard
2. Mark a paper as wanted
3. Click "Find PDF"
4. Click "Summarize"

---

## Troubleshooting

### "API Not Connected"
- Check Render logs for errors
- Verify the API URL is correct in dashboard.html
- Check CORS settings in `agents/api.py` include your domain

### "Credit balance is too low"
- You set `ANTHROPIC_API_KEY` - remove it
- Only `CLAUDE_CODE_OAUTH_TOKEN` should be set

### Summarization hangs
- Render free tier may timeout long requests (30s)
- Consider upgrading to paid tier for longer timeouts

### Data not persisting
- Verify disk is mounted at `/var/data`
- Check `VAULT_PATH` environment variable is `/var/data/vault`

---

## Alternative: Minimal Local Deployment

If you just want to show your existing summaries publicly (read-only):

1. Generate static JSON from your vault:
   ```python
   # scripts/export_summaries.py
   import json
   from agents.vault import VaultManager

   vault = VaultManager("./vault")
   summaries = []
   for paper in vault.index.papers.values():
       if paper.status == "summarized":
           summaries.append(paper.model_dump())

   with open("summaries.json", "w") as f:
       json.dump(summaries, f)
   ```

2. Host `summaries.json` and a static viewer on your Hugo site

3. No backend needed - pure static hosting

---

## Costs

| Component | Cost |
|-----------|------|
| Render Free Tier | $0 (750 hours/month, sleeps after inactivity) |
| Render Starter | $7/month (always on, more resources) |
| Render Disk (1GB) | $0.25/month |
| Hugo Hosting | Free (Netlify, GitHub Pages, etc.) |
| Summarization | Included with Claude Pro/Max subscription |

---

## Future: Multi-User Support

For a true multi-user platform, you'll need:

1. **Authentication**: Add user accounts (OAuth, magic links, etc.)
2. **Token Storage**: Users provide their own Claude OAuth tokens
3. **Database**: PostgreSQL for user data, paper metadata
4. **File Storage**: S3/R2 for PDFs instead of local disk
5. **Job Queue**: Redis/Celery for background processing

This is a significant expansion beyond the current single-user design.
