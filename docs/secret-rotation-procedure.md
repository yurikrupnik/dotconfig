# Secret Rotation Procedure

## Rotation Schedule
All secrets should be rotated every **90 days** (3 months).

## Secrets to Rotate
- Docker Hub password
- GitHub token (Personal Access Token)
- [Add other secrets here]

## Rotation Steps

### 1. Docker Hub Password
```bash
# 1. Change password on Docker Hub
# 2. Update in GCP Secret Manager
echo "NEW_PASSWORD" | gcloud secrets versions add docker-password \
    --data-file=- \
    --project=YOUR_PROJECT_ID

# 3. Mark as rotated
nu scripts/nu/secrets.nu mark-rotated docker-password --project=YOUR_PROJECT_ID
```

### 2. GitHub Token
```bash
# 1. Generate new token at https://github.com/settings/tokens
# 2. Update in GCP Secret Manager
echo "ghp_NEW_TOKEN" | gcloud secrets versions add github-token \
    --data-file=- \
    --project=YOUR_PROJECT_ID

# 3. Mark as rotated
nu scripts/nu/secrets.nu mark-rotated github-token --project=YOUR_PROJECT_ID

# 4. Revoke old token on GitHub
```

### 3. Verify All Secrets
```bash
# Fetch and test new secrets
nu scripts/nu/secrets.nu fetch
source .env

# Test Docker login
echo $DOCKER_PASSWORD | docker login -u YOUR_USERNAME --password-stdin

# Test GitHub token
gh auth status
```

## Automation
- Set calendar reminder every 3 months
- Check rotation status: `nu scripts/nu/secrets.nu check-rotation`
- Cloud Scheduler job sends monthly reminders

## Security Notes
- Always revoke old credentials after rotation
- Test new credentials before revoking old ones
- Keep rotation log in this document

## Rotation Log
| Secret | Last Rotated | Next Rotation | Rotated By |
|--------|--------------|---------------|------------|
| docker-password | YYYY-MM-DD | YYYY-MM-DD | |
| github-token | YYYY-MM-DD | YYYY-MM-DD | |
