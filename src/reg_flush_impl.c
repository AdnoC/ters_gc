#include <string.h>
#include <setjmp.h>

typedef void (*callback_func)(void *);

void flush_registers_and_call(callback_func callback, void *data) {
  jmp_buf env;
  memset(&env, 0, sizeof(jmp_buf));
  setjmp(env);

  callback(data);
}
