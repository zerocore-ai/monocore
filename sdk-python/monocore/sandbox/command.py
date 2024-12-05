from typing import List, Optional


class CommandExecutor:
    async def execute(
        self, command: str, args: List[str] = [], timeout: Optional[float] = None
    ) -> None: ...
