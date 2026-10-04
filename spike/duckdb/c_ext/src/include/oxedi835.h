/* C interface of the oxedi835_ffi static library. */
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct Oxedi835Table Oxedi835Table;

enum { OXEDI835_BINARY = 0, OXEDI835_INT64 = 1, OXEDI835_DECIMAL128 = 2, OXEDI835_DATE32 = 3, OXEDI835_TIME32 = 4 };

typedef struct {
	uint32_t kind;
	uint8_t precision;
	uint8_t scale;
	const uint8_t *validity;
	const int32_t *offsets;
	const uint8_t *data;
} Oxedi835Column;

Oxedi835Table *oxedi835_parse(const uint8_t *bytes, size_t len, const char *table, char **error);
size_t oxedi835_table_rows(const Oxedi835Table *handle);
size_t oxedi835_table_columns(const Oxedi835Table *handle);
const char *oxedi835_table_column_name(const Oxedi835Table *handle, size_t column);
bool oxedi835_table_column(const Oxedi835Table *handle, size_t column, Oxedi835Column *out);
void oxedi835_table_free(Oxedi835Table *handle);
void oxedi835_string_free(char *text);
