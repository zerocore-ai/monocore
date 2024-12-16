from typing import Any, Callable


class StdIO:
    def on_change(self, stream: str, callback: Callable[[Any], None]) -> None: ...
