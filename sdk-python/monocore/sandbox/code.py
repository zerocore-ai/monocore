from typing import Optional


class CodeExecutor:
    async def execute(
        self, code: str, stream: bool = False, timeout: Optional[float] = None
    ) -> None: ...
