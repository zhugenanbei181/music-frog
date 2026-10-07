"""Small Rust lexical masker for structural quality rules, never behavior evidence."""
from __future__ import annotations

import re


TOKEN = re.compile(r'//[^\n]*|/\*|(?:br|r)(?P<hashes>\#*)"|(?:b)?"|(?:b)?\'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.)|[^\'\\\n])\'')


def _mask(text: str, literals: bool) -> str:
    result = list(text)
    position = 0
    while match := TOKEN.search(text, position):
        start, end = match.span()
        token = match.group()
        if token == '/*':
            depth = 1
            while depth and end < len(text):
                marker = re.search(r'/\*|\*/', text[end:])
                if marker is None:
                    end = len(text)
                    break
                end += marker.end()
                depth += 1 if marker.group() == '/*' else -1
        elif not token.startswith('//') and token.endswith('"'):
            hashes = match.group('hashes')
            if hashes is not None:
                finish = text.find('"' + hashes, end)
                end = len(text) if finish < 0 else finish + len(hashes) + 1
            else:
                while end < len(text):
                    if text[end] == '\\':
                        end += 2
                    elif text[end] == '"':
                        end += 1
                        break
                    else:
                        end += 1
        if literals or token == '/*' or token.startswith('//'):
            for index in range(start, min(end, len(text))):
                if text[index] != '\n':
                    result[index] = ' '
        position = end
    return ''.join(result)


def mask_noncode(text: str) -> str:
    return _mask(text, literals=True)


def mask_comments(text: str) -> str:
    """Ignore comments while counting literals and their continuation lines as code."""
    return _mask(text, literals=False)


def mask_uses(text: str) -> str:
    return re.sub(r'\buse\s+[^;]+;', lambda match: ''.join('\n' if char == '\n' else ' ' for char in match.group()), text, flags=re.S)


LONG_PATH = re.compile(r'(?<![\w:])(?:::)?(?:crate|super|self|[a-z_][A-Za-z0-9_]*)\s*::\s*[a-z_][A-Za-z0-9_]*\s*::\s*[A-Za-z_][A-Za-z0-9_]*(?:\s*::\s*[A-Za-z_][A-Za-z0-9_]*)*')


def long_paths(text: str):
    return list(LONG_PATH.finditer(mask_uses(mask_noncode(text))))


def mask_test_items(text: str) -> str:
    """Mask cfg(test) items without hiding production that follows them."""
    code = list(text)
    syntax = mask_noncode(text)
    for attribute in re.finditer(r'#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]', syntax):
        start = attribute.start()
        position = attribute.end()
        while match := re.match(r'\s*#\s*\[', syntax[position:]):
            position += match.end()
            depth = 1
            while depth and position < len(syntax):
                depth += (syntax[position] == '[') - (syntax[position] == ']')
                position += 1
        depth = 0
        for position in range(position, len(syntax)):
            char = syntax[position]
            if char == '{':
                depth += 1
            elif char == '}':
                depth -= 1
                if depth == 0:
                    position += 1
                    break
            elif char == ';' and depth == 0:
                position += 1
                break
        for index in range(start, position):
            if code[index] != '\n':
                code[index] = ' '
    return ''.join(code)
