"""OAuth authentication for Marginalia."""

import json
import os
import secrets
from datetime import datetime, timedelta
from pathlib import Path
from typing import Optional

from authlib.integrations.starlette_client import OAuth
from fastapi import Depends, HTTPException, Request
from fastapi.security import HTTPBearer
from jose import jwt
from pydantic import BaseModel, Field

# Configuration
SECRET_KEY = os.getenv("SECRET_KEY", secrets.token_hex(32))
ALGORITHM = "HS256"
ACCESS_TOKEN_EXPIRE_HOURS = 24 * 7  # 1 week


class User(BaseModel):
    """User account."""
    id: str = Field(default_factory=lambda: secrets.token_hex(16))
    email: str
    name: Optional[str] = None
    picture: Optional[str] = None
    provider: str  # "google" or "github"
    provider_id: str
    created_at: datetime = Field(default_factory=datetime.now)

    # Settings
    claude_oauth_key: Optional[str] = None
    storage_path: Optional[str] = None
    setup_complete: bool = False


# OAuth setup
oauth = OAuth()


def configure_oauth():
    """Configure OAuth providers. Call after loading env vars."""
    google_client_id = os.getenv('GOOGLE_CLIENT_ID')
    google_client_secret = os.getenv('GOOGLE_CLIENT_SECRET')
    github_client_id = os.getenv('GITHUB_CLIENT_ID')
    github_client_secret = os.getenv('GITHUB_CLIENT_SECRET')

    if google_client_id and google_client_secret:
        oauth.register(
            name='google',
            client_id=google_client_id,
            client_secret=google_client_secret,
            server_metadata_url='https://accounts.google.com/.well-known/openid-configuration',
            client_kwargs={'scope': 'openid email profile'},
        )

    if github_client_id and github_client_secret:
        oauth.register(
            name='github',
            client_id=github_client_id,
            client_secret=github_client_secret,
            access_token_url='https://github.com/login/oauth/access_token',
            authorize_url='https://github.com/login/oauth/authorize',
            api_base_url='https://api.github.com/',
            client_kwargs={'scope': 'read:user user:email'},
        )


class UserStore:
    """Simple file-based user storage."""

    def __init__(self, storage_path: Optional[Path] = None):
        self.storage_path = storage_path or Path(os.getenv("VAULT_PATH", "./vault"))
        self.users_file = self.storage_path / ".users.json"
        self.users: dict[str, User] = {}
        self.load()

    def load(self):
        """Load users from disk."""
        if self.users_file.exists():
            try:
                with open(self.users_file) as f:
                    data = json.load(f)
                    self.users = {
                        k: User.model_validate(v)
                        for k, v in data.get('users', {}).items()
                    }
            except Exception as e:
                print(f"Error loading users: {e}")
                self.users = {}

    def save(self):
        """Save users to disk."""
        self.storage_path.mkdir(parents=True, exist_ok=True)
        with open(self.users_file, 'w') as f:
            json.dump({
                'users': {k: v.model_dump(mode='json') for k, v in self.users.items()}
            }, f, indent=2, default=str)

    def get_or_create_user(
        self,
        provider: str,
        provider_id: str,
        email: str,
        name: Optional[str] = None,
        picture: Optional[str] = None
    ) -> User:
        """Get existing user or create new one."""
        key = f"{provider}:{provider_id}"
        if key not in self.users:
            self.users[key] = User(
                email=email,
                name=name,
                picture=picture,
                provider=provider,
                provider_id=provider_id,
            )
            self.save()
        else:
            # Update name/picture if changed
            user = self.users[key]
            if name and name != user.name:
                user.name = name
            if picture and picture != user.picture:
                user.picture = picture
            self.save()
        return self.users[key]

    def get_user_by_id(self, user_id: str) -> Optional[User]:
        """Get a user by their ID."""
        for user in self.users.values():
            if user.id == user_id:
                return user
        return None

    def update_user(self, user: User):
        """Update a user's data."""
        key = f"{user.provider}:{user.provider_id}"
        self.users[key] = user
        self.save()


# Global user store instance
user_store: Optional[UserStore] = None


def get_user_store() -> UserStore:
    """Get the global user store, initializing if needed."""
    global user_store
    if user_store is None:
        user_store = UserStore()
    return user_store


def create_access_token(user_id: str) -> str:
    """Create a JWT access token."""
    expire = datetime.utcnow() + timedelta(hours=ACCESS_TOKEN_EXPIRE_HOURS)
    data = {"sub": user_id, "exp": expire}
    return jwt.encode(data, SECRET_KEY, algorithm=ALGORITHM)


def verify_token(token: str) -> Optional[str]:
    """Verify a JWT token and return the user ID."""
    try:
        payload = jwt.decode(token, SECRET_KEY, algorithms=[ALGORITHM])
        return payload.get("sub")
    except Exception:
        return None


# FastAPI security
security = HTTPBearer(auto_error=False)


async def get_current_user(
    request: Request,
    credentials=Depends(security)
) -> Optional[User]:
    """Get current user from JWT token (header or cookie)."""
    token = None

    # Check Authorization header first
    if credentials:
        token = credentials.credentials

    # Also check cookie
    if not token:
        token = request.cookies.get("marginalia_token")

    if not token:
        return None

    user_id = verify_token(token)
    if not user_id:
        return None

    return get_user_store().get_user_by_id(user_id)


async def require_auth(user: Optional[User] = Depends(get_current_user)) -> User:
    """Require an authenticated user."""
    if not user:
        raise HTTPException(status_code=401, detail="Not authenticated")
    return user


async def require_setup_complete(user: User = Depends(require_auth)) -> User:
    """Require an authenticated user with completed setup."""
    if not user.setup_complete:
        raise HTTPException(status_code=403, detail="Setup not complete")
    return user


def is_oauth_configured() -> bool:
    """Check if any OAuth provider is configured."""
    return bool(
        os.getenv('GOOGLE_CLIENT_ID') or os.getenv('GITHUB_CLIENT_ID')
    )
