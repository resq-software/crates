/*
 * Small demo binary for resq-bin analysis.
 *
 * Compile: gcc -o demo demo.c -O2 -g
 * Analyze: cargo run -p resq-bin -- --file ./demo --plain
 *
 * This program includes enough structure (multiple functions, static data,
 * stack operations) to produce interesting disassembly output.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Global data — appears in the .data / .rodata sections */
static const char *GREETING = "Hello from resq-bin demo!";
static int counter = 0;

/* A simple struct to generate interesting symbol info */
typedef struct {
    int id;
    char name[32];
    double score;
} Record;

/* Fibonacci — recursive, generates a non-trivial call stack */
int fibonacci(int n) {
    if (n <= 1) return n;
    return fibonacci(n - 1) + fibonacci(n - 2);
}

/* Iterative sum — loop-based, different instruction patterns */
long sum_to(long n) {
    long total = 0;
    for (long i = 1; i <= n; i++) {
        total += i;
    }
    return total;
}

/* String manipulation — uses libc, generates PLT entries */
void process_record(Record *rec) {
    snprintf(rec->name, sizeof(rec->name), "record-%04d", rec->id);
    rec->score = (double)rec->id * 3.14159;
    counter++;
}

/* Array operations — heap allocation, pointer arithmetic */
Record *create_records(int count) {
    Record *records = calloc(count, sizeof(Record));
    if (!records) return NULL;

    for (int i = 0; i < count; i++) {
        records[i].id = i + 1;
        process_record(&records[i]);
    }
    return records;
}

/* Print summary — multiple format strings, stdout interaction */
void print_summary(Record *records, int count) {
    printf("%s\n\n", GREETING);
    printf("Generated %d records:\n", count);

    for (int i = 0; i < count && i < 5; i++) {
        printf("  #%d: %-20s score=%.2f\n",
               records[i].id, records[i].name, records[i].score);
    }

    if (count > 5) {
        printf("  ... and %d more\n", count - 5);
    }

    printf("\nFibonacci(10) = %d\n", fibonacci(10));
    printf("Sum(1..100)   = %ld\n", sum_to(100));
    printf("Total records processed: %d\n", counter);
}

int main(int argc, char *argv[]) {
    int count = 10;
    if (argc > 1) {
        count = atoi(argv[1]);
        if (count <= 0 || count > 1000) count = 10;
    }

    Record *records = create_records(count);
    if (!records) {
        fprintf(stderr, "Failed to allocate records\n");
        return 1;
    }

    print_summary(records, count);
    free(records);
    return 0;
}
