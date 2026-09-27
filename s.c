#include <sys/proc_info.h>
#include <stdio.h>
int main(){printf("%zu %zu\n", sizeof(struct proc_uniqidentifierinfo), sizeof(struct proc_bsdinfo));}
