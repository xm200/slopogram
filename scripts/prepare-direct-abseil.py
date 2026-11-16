#!/usr/bin/env python3
from pathlib import Path

path = Path(__file__).resolve().parent.parent / 'native/telegram-macos/submodules/tg_owt/src/third_party/abseil-cpp/absl/base/attributes.h'
text = path.read_text()
for definition in ('#define ABSL_ATTRIBUTE_LIFETIME_BOUND [[clang::lifetimebound]]', '#define ABSL_ATTRIBUTE_LIFETIME_BOUND __attribute__((lifetimebound))'):
    text = text.replace(definition, '#define ABSL_ATTRIBUTE_LIFETIME_BOUND')
path.write_text(text)
