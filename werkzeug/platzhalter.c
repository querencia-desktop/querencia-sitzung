// Platzhalter für Sitzungsprogramme in der Abmelde-Prüfung (G2-AI, QD-062):
// wird unter dem Namen eines Sitzungsprogramms kopiert (querencia-netzwerk,
// querencia-polkit-agent, …) und wartet nur. Mit dem Argument „taub“
// ignoriert er SIGTERM, damit der SIGKILL-Weg geprüft werden kann.
#include <signal.h>
#include <string.h>
#include <unistd.h>

int main(int argc, char **argv) {
    if (argc > 1 && strcmp(argv[1], "taub") == 0)
        signal(SIGTERM, SIG_IGN);
    for (;;)
        pause();
}
