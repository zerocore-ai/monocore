from typing import List

from .base import BaseSandbox


class Sandbox(BaseSandbox):
    def __init__(self, base: str, env: List[str] = []) -> None:
        super().__init__()
        self.base = base
        self.env = env
