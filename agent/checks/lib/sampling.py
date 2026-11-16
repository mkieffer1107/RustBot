"""Deterministic sampling of conversations (same rule for goldens builds and --quick checks)."""
import zlib


def in_sample(conv_id, percent):
    return percent >= 100 or zlib.crc32(conv_id.encode("utf-8")) % 100 < percent
