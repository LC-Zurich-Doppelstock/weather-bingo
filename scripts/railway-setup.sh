#!/usr/bin/env bash
#
# Railway deployment setup script for Weather Bingo.
#
# Prerequisites:
#   1. Railway CLI installed: brew install railway
#   2. Logged in: railway login
#   3. GitHub repo pushed: github.com/LC-Zurich-Doppelstock/weather-bingo
#
# Usage:
#   ./scripts/railway-setup.sh
#
# What this script does:
#   1. Creates a new Railway project
#   2. Provisions a PostgreSQL database
#   3. Creates the API service (from GitHub repo, Dockerfile builder)
#   4. Creates the frontend service (from GitHub repo, Dockerfile builder)
#   5. Sets all environment variables
#   6. Generates a public domain for the frontend
#
# After running, push to main to trigger the first deployment.
#
set -euo pipefail

REPO="LC-Zurich-Doppelstock/weather-bingo"
PROJECT_NAME="weather-bingo"

# Colours for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No colour

info()  { echo -e "${CYAN}[INFO]${NC}  $*"; }
ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*"; exit 1; }

# ── Preflight checks ─────────────────────────────────────────────

command -v railway >/dev/null 2>&1 || error "Railway CLI not found. Install with: brew install railway"

railway whoami >/dev/null 2>&1 || error "Not logged in. Run: railway login"

info "Logged in as: $(railway whoami 2>/dev/null)"

# ── Step 1: Create project ───────────────────────────────────────

info "Creating Railway project '${PROJECT_NAME}'..."
railway init --name "${PROJECT_NAME}" -y 2>/dev/null || true
ok "Project created (or already linked)"

# ── Step 2: Add PostgreSQL ───────────────────────────────────────

info "Adding PostgreSQL database..."
railway add --database postgres -y 2>/dev/null
ok "PostgreSQL provisioned"

# ── Step 3: Create API service ───────────────────────────────────

info "Adding API service from GitHub repo..."
railway add --repo "${REPO}" -y 2>/dev/null
# The service is created — we need to link to it to configure it.
# Railway CLI requires linking to a service before setting variables.

info "Linking to API service..."
echo ""
warn "Railway will prompt you to select a service."
warn "Select the service that was just created (NOT PostgreSQL)."
warn "We will rename it to 'api'."
echo ""
railway service

# Set variables for the API service
info "Setting API environment variables..."
railway variable set \
  "YR_USER_AGENT=WeatherBingo/0.1 github.com/LC-Zurich-Doppelstock/weather-bingo" \
  "DATA_DIR=/app/data" \
  -y 2>/dev/null
ok "API variables set (YR_USER_AGENT, DATA_DIR)"

echo ""
warn "MANUAL STEP REQUIRED:"
warn "  1. Open the Railway dashboard: railway open"
warn "  2. Click on the API service → Variables tab"
warn "  3. Add DATABASE_URL as a reference variable:"
warn "     Click '+ New Variable' → 'Add Reference' → select PostgreSQL → DATABASE_URL"
warn "  4. In Settings → Build → Config File Path, set: api/railway.toml"
echo ""
read -rp "Press Enter when done..."

ok "API service configured"

# ── Step 4: Create frontend service ──────────────────────────────

info "Adding frontend service from GitHub repo..."
railway add --repo "${REPO}" -y 2>/dev/null

info "Linking to frontend service..."
echo ""
warn "Select the NEW service (not the API or PostgreSQL)."
echo ""
railway service

# Set variables for the frontend service
info "Setting frontend environment variables..."
railway variable set \
  "API_URL=http://api.railway.internal:8080" \
  -y 2>/dev/null
ok "Frontend variables set (API_URL)"

echo ""
warn "MANUAL STEP REQUIRED:"
warn "  1. Open the Railway dashboard: railway open"
warn "  2. Click on the frontend service → Settings → Build"
warn "  3. Set Config File Path to: frontend/railway.toml"
echo ""
read -rp "Press Enter when done..."

# Generate public domain for the frontend
info "Generating public domain for frontend..."
railway domain 2>/dev/null
ok "Public domain generated"

# ── Done ─────────────────────────────────────────────────────────

echo ""
echo -e "${GREEN}════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}  Railway setup complete!${NC}"
echo -e "${GREEN}════════════════════════════════════════════════════${NC}"
echo ""
info "Next steps:"
echo "  1. Open the dashboard to verify: railway open"
echo "  2. Push to main to trigger deployment: git push origin main"
echo "  3. Watch build logs: railway logs --build"
echo "  4. The API build will take 5-15 minutes (Rust compilation)"
echo ""
info "Troubleshooting:"
echo "  - Check API logs:      railway logs -s api"
echo "  - Check frontend logs: railway logs -s frontend"
echo "  - SSH into a service:  railway ssh -s api"
echo "  - Connect to DB:       railway connect"
echo ""
