#include <stdlib.h>
#include "HsFFI.h"

static void library_init(void) __attribute__((constructor));
static void library_init(void) {
    static int argc = 1;

    // hs_init expects (int*, char***). argv must be an array of C strings.
    static const char *argv_const[] = { "libducklingffi.so", NULL };

    // Cast away const only to satisfy hs_init's signature.
    static char **argv = (char **)argv_const;

    hs_init(&argc, &argv);
}

static void library_exit(void) __attribute__((destructor));
static void library_exit(void) {
    hs_exit();
}