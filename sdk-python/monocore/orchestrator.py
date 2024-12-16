from types import TracebackType
from typing import Any, List, Optional

from typing_extensions import Self

from .sandbox import BaseSandbox


class Service:
    def __init__(self, name: str, base: str) -> None:
        self.name = name
        self.base = base


class Orchestrator:
    def __init__(
        self, services: Optional[List[Service]] = None, groups: List[Any] = [], **kwargs
    ) -> None:
        self.services = services or []
        self.groups = groups
        self.config = kwargs.get("from")

    async def __aenter__(self) -> Self:
        return self

    async def __aexit__(
        self,
        exc_type: Optional[type[BaseException]],
        exc_val: Optional[BaseExceptio],
        exc_tb: Optional[TracebackType],
    ) -> None: ...

    async def list(self) -> List[BaseSandbox]: ...

    async def get(self, name: str) -> BaseSandbox: ...
