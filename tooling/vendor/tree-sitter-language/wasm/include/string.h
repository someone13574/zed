#ifndef TREE_SITTER_WASM_STRING_H_
#define TREE_SITTER_WASM_STRING_H_

#include <stdint.h>

int memcmp(const void *lhs, const void *rhs, size_t count);

void *memcpy(void *restrict dst, const void *restrict src, size_t size);

void *memmove(void *dst, const void *src, size_t count);

void *memset(void *dst, int value, size_t count);

size_t strlen(const char *str);

char *strncpy(char *dest, const char *src, size_t n);

int strncmp(const char *left, const char *right, size_t n);

#endif // TREE_SITTER_WASM_STRING_H_
