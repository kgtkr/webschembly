#include <stdio.h>

int prime_count(int n) {
    int count = 0;
    for (int i = 2; i <= n; i++) {
        int is_prime = 1;
        for (int j = 2; j < i; j++) {
            is_prime &= i % j != 0;
        }
        if (is_prime) {
            count++;
        }
    }
    return count;
}


int main() {
    printf("%d\n", prime_count(10000));
    return 0;
}
