"""Python bindings for legacy MQL parsing and MQL 0.2 semantic IR validation."""

from typing import Dict

__version__: str

class MQL:
    """Structured output from the retained legacy MQL parser."""
    def version(self) -> str: ...
    def json(self) -> str: ...
    def json_pretty(self) -> str: ...

class SemanticIR:
    """Validated MQL 0.2 semantic IR document."""
    def version(self) -> str: ...
    def json(self) -> str: ...
    def json_pretty(self) -> str: ...

def parse(mql: str) -> MQL:
    """
    Parse MQL input to a structured output.
    This can be formatted as JSON, pretty-printed JSON, or the internal Rust struct with `str`
    """
    ...

def parse_with_externals(mql: str, externals: Dict[str, str]) -> MQL:
    """
    Parse MQL input with external variable substitutions.
    
    Args:
        mql: The MQL query string to parse
        externals: Dictionary mapping variable names (including $ sigil) to their values as MQL expression strings
        
    Returns:
        MQL object containing the parsed result
        
    Example:
        output = pylibmql.parse_with_externals(
            'extern $course; SELECT 1 FROM $course: "Take the course";',
            {"$course": "MATH 2250"}
        )
    """
    ...

def parse_extern_value(expr: str) -> str:
    """
    Parse an MQL expression string into a value suitable for use as an extern variable.
    
    Accepts the same syntax as variable assignment values:
    - "MATH 2250" → Class selector
    - "[MATH 2250, MATH 2260]" → Selector list
    - '"some string"' → String value
    - "3" → Quantity value
    
    Args:
        expr: MQL expression string to parse
        
    Returns:
        Internal representation of the parsed value (for debugging/inspection)
    """
    ...

def validate_semantic_ir(json: str) -> SemanticIR:
    """Deserialize and semantically validate a versioned MQL 0.2 IR document."""
    ...
