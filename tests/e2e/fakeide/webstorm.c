#include <unistd.h>
#include <sys/wait.h>
int main(void) {
  pid_t p = fork();
  if (p == 0) { execlp("node", "node", "--inspect=127.0.0.1:0", "-e", "setTimeout(()=>{},30000)", (char*)0); _exit(1); }
  int st; waitpid(p, &st, 0); return 0;
}
