#pragma once
#include <stdint.h>
#include <stddef.h>
#include <string>
#include <cstdlib>
#include <cstring>
#define F(x) (x)
#define A0 14
using byte = uint8_t;
struct FakeSerial {
    std::string output;
    void begin(long) {}
    void print(const char* s) { output += s; }
    void print(char c) { output += c; }
    void print(int n) { output += std::to_string(n); }
    void println(const char* s) { output += s; output += '\n'; }
    void println() { output += '\n'; }
    void write(char c) { output += c; }
    int available() { return 0; }
    int read() { return -1; }
} ;
extern FakeSerial Serial;
inline long random(long max) { return max > 0 ? std::rand() % max : 0; }
inline long random(long min, long max) { return min + random(max-min); }
inline void randomSeed(unsigned long n) { std::srand(n); }
inline int analogRead(uint8_t) { return 531; }
inline unsigned long micros() { return 123456; }
