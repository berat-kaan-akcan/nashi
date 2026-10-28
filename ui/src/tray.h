#pragma once

#include <QObject>

class App;
class QAction;
class QMenu;
class QSystemTrayIcon;

// Tray icon (StatusNotifierItem on Plasma).
class Tray : public QObject
{
    Q_OBJECT

public:
    explicit Tray(App *app, QObject *parent = nullptr);
    ~Tray() override;

private:
    void retranslate();
    void syncChords();

    App *m_app;
    QSystemTrayIcon *m_icon = nullptr;
    QMenu *m_menu = nullptr;
    QAction *m_open = nullptr;
    QAction *m_chords = nullptr;
    QAction *m_quit = nullptr;
};
