from types import TracebackType
from typing import AsyncIterator, Optional

from typing_extensions import Self

from .code import CodeExecutor
from .command import CommandExecutor
from .filesystem import FileSystem
from .stdio import StdIO


class BaseSandbox:
    def __init__(self) -> None:
        self.fs = FileSystem()
        self.code = CodeExecutor()
        self.stdio = StdIO()
        self.command = CommandExecutor()

    async def __aenter__(self) -> Self:
        return self

    async def __aexit__(
        self,
        exc_type: Optional[type[BaseException]],
        exc_val: Optional[BaseException],
        exc_tb: Optional[TracebackType],
    ) -> None: ...

    async def output(self) -> str: ...

    async def logs(self) -> str: ...

    async def error_stream(self) -> AsyncIterator[str]: ...

    async def output_stream(self) -> AsyncIterator[str]: ...
