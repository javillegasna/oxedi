/*
 * read_835(path, table_name := 'claims') over the stable DuckDB C extension API.
 * Parsing and projection happen in the oxedi835_ffi static library; this file
 * only declares the result columns and copies rows into DuckDB vectors.
 */
#include "duckdb_extension.h"
#include "oxedi835.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

DUCKDB_EXTENSION_EXTERN

typedef struct {
	Oxedi835Table *table;
} BindData;

typedef struct {
	idx_t next_row;
} InitData;

static void destroy_bind(void *data) {
	BindData *bind = (BindData *)data;
	oxedi835_table_free(bind->table);
	duckdb_free(bind);
}

static char *read_file(const char *path, size_t *len) {
	FILE *file = fopen(path, "rb");
	if (!file) {
		return NULL;
	}
	fseek(file, 0, SEEK_END);
	long size = ftell(file);
	fseek(file, 0, SEEK_SET);
	char *bytes = malloc(size > 0 ? (size_t)size : 1);
	*len = bytes ? fread(bytes, 1, (size_t)size, file) : 0;
	fclose(file);
	return bytes;
}

static duckdb_logical_type column_type(const Oxedi835Column *column) {
	switch (column->kind) {
	case OXEDI835_BINARY:
		return duckdb_create_logical_type(DUCKDB_TYPE_BLOB);
	case OXEDI835_INT64:
		return duckdb_create_logical_type(DUCKDB_TYPE_BIGINT);
	case OXEDI835_DECIMAL128:
		return duckdb_create_decimal_type(column->precision, column->scale);
	case OXEDI835_DATE32:
		return duckdb_create_logical_type(DUCKDB_TYPE_DATE);
	default:
		return duckdb_create_logical_type(DUCKDB_TYPE_TIME);
	}
}

static void read_835_bind(duckdb_bind_info info) {
	duckdb_value path_value = duckdb_bind_get_parameter(info, 0);
	char *path = duckdb_get_varchar(path_value);
	duckdb_destroy_value(&path_value);

	duckdb_value table_value = duckdb_bind_get_named_parameter(info, "table_name");
	char *table_name = table_value ? duckdb_get_varchar(table_value) : NULL;
	if (table_value) {
		duckdb_destroy_value(&table_value);
	}

	size_t len = 0;
	char *bytes = read_file(path, &len);
	if (!bytes) {
		char message[512];
		snprintf(message, sizeof(message), "read_835: cannot read %s", path);
		duckdb_bind_set_error(info, message);
		duckdb_free(path);
		duckdb_free(table_name);
		return;
	}
	char *error = NULL;
	Oxedi835Table *table = oxedi835_parse((const uint8_t *)bytes, len, table_name ? table_name : "claims", &error);
	free(bytes);
	duckdb_free(table_name);
	if (!table) {
		char message[1024];
		snprintf(message, sizeof(message), "read_835: %s: %s", path, error ? error : "parse failed");
		duckdb_bind_set_error(info, message);
		oxedi835_string_free(error);
		duckdb_free(path);
		return;
	}
	duckdb_free(path);

	size_t columns = oxedi835_table_columns(table);
	for (size_t i = 0; i < columns; i++) {
		Oxedi835Column column;
		oxedi835_table_column(table, i, &column);
		duckdb_logical_type type = column_type(&column);
		duckdb_bind_add_result_column(info, oxedi835_table_column_name(table, i), type);
		duckdb_destroy_logical_type(&type);
	}
	duckdb_bind_set_cardinality(info, oxedi835_table_rows(table), true);

	BindData *bind = (BindData *)duckdb_malloc(sizeof(BindData));
	bind->table = table;
	duckdb_bind_set_bind_data(info, bind, destroy_bind);
}

static void read_835_init(duckdb_init_info info) {
	InitData *init = (InitData *)duckdb_malloc(sizeof(InitData));
	init->next_row = 0;
	duckdb_init_set_init_data(info, init, duckdb_free);
}

static bool is_valid(const uint8_t *validity, idx_t row) {
	return !validity || (validity[row >> 3] >> (row & 7)) & 1;
}

static void fill(duckdb_vector vector, const Oxedi835Column *column, idx_t start, idx_t count) {
	void *out = duckdb_vector_get_data(vector);
	switch (column->kind) {
	case OXEDI835_BINARY:
		for (idx_t i = 0; i < count; i++) {
			int32_t from = column->offsets[start + i];
			int32_t to = column->offsets[start + i + 1];
			duckdb_vector_assign_string_element_len(vector, i, (const char *)column->data + from, (idx_t)(to - from));
		}
		break;
	case OXEDI835_INT64:
		memcpy(out, (const int64_t *)column->data + start, count * sizeof(int64_t));
		break;
	case OXEDI835_DECIMAL128: {
		const __int128 *values = (const __int128 *)column->data + start;
		for (idx_t i = 0; i < count; i++) {
			if (column->precision <= 4) {
				((int16_t *)out)[i] = (int16_t)values[i];
			} else if (column->precision <= 9) {
				((int32_t *)out)[i] = (int32_t)values[i];
			} else if (column->precision <= 18) {
				((int64_t *)out)[i] = (int64_t)values[i];
			} else {
				duckdb_hugeint value;
				value.lower = (uint64_t)values[i];
				value.upper = (int64_t)(values[i] >> 64);
				((duckdb_hugeint *)out)[i] = value;
			}
		}
		break;
	}
	case OXEDI835_DATE32:
		memcpy(out, (const int32_t *)column->data + start, count * sizeof(int32_t));
		break;
	default:
		for (idx_t i = 0; i < count; i++) {
			((int64_t *)out)[i] = (int64_t)((const int32_t *)column->data)[start + i] * 1000000;
		}
		break;
	}
	if (column->validity) {
		duckdb_vector_ensure_validity_writable(vector);
		uint64_t *mask = duckdb_vector_get_validity(vector);
		for (idx_t i = 0; i < count; i++) {
			if (!is_valid(column->validity, start + i)) {
				duckdb_validity_set_row_invalid(mask, i);
			}
		}
	}
}

static void read_835_scan(duckdb_function_info info, duckdb_data_chunk output) {
	BindData *bind = (BindData *)duckdb_function_get_bind_data(info);
	InitData *init = (InitData *)duckdb_function_get_init_data(info);
	idx_t rows = oxedi835_table_rows(bind->table);
	idx_t start = init->next_row;
	idx_t count = rows - start;
	if (count > duckdb_vector_size()) {
		count = duckdb_vector_size();
	}
	size_t columns = oxedi835_table_columns(bind->table);
	for (size_t i = 0; i < columns; i++) {
		Oxedi835Column column;
		oxedi835_table_column(bind->table, i, &column);
		fill(duckdb_data_chunk_get_vector(output, i), &column, start, count);
	}
	init->next_row = start + count;
	duckdb_data_chunk_set_size(output, count);
}

DUCKDB_EXTENSION_ENTRYPOINT(duckdb_connection connection, duckdb_extension_info info,
                            struct duckdb_extension_access *access) {
	duckdb_table_function function = duckdb_create_table_function();
	duckdb_table_function_set_name(function, "read_835");
	duckdb_logical_type varchar = duckdb_create_logical_type(DUCKDB_TYPE_VARCHAR);
	duckdb_table_function_add_parameter(function, varchar);
	duckdb_table_function_add_named_parameter(function, "table_name", varchar);
	duckdb_destroy_logical_type(&varchar);
	duckdb_table_function_set_bind(function, read_835_bind);
	duckdb_table_function_set_init(function, read_835_init);
	duckdb_table_function_set_function(function, read_835_scan);
	duckdb_state state = duckdb_register_table_function(connection, function);
	duckdb_destroy_table_function(&function);
	if (state != DuckDBSuccess) {
		access->set_error(info, "read_835 could not be registered");
		return false;
	}
	return true;
}
