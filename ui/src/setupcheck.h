#pragma once

#include <QList>
#include <QObject>
#include <QString>
#include <QtQmlIntegration/qqmlintegration.h>

class QSettings;

// First-run state and the local system checks shown by WelcomeWindow.qml:
// whether this session may read /dev/input (via the `input` group, which
// nashi-inputd needs) and the commands the user can run to fix it. Nothing
// here runs a command; the app only shows them for copying.
class SetupCheck : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Provided by the application")

    Q_PROPERTY(bool welcomeDone READ welcomeDone WRITE setWelcomeDone NOTIFY welcomeDoneChanged)
    Q_PROPERTY(GroupState groupState READ groupState NOTIFY changed)
    Q_PROPERTY(bool inInputGroup READ inInputGroup NOTIFY changed)
    Q_PROPERTY(QString userName READ userName NOTIFY changed)
    Q_PROPERTY(QString groupCommand READ groupCommand CONSTANT)
    Q_PROPERTY(QString serviceCommand READ serviceCommand CONSTANT)

public:
    enum GroupState {
        InGroup,      // this session already has the group: chords can work
        NeedsRelogin, // the account was added, but this session predates it
        NotInGroup,   // the account is not a member yet
        NoGroup,      // the system has no `input` group at all
    };
    Q_ENUM(GroupState)

    static inline const QString kWelcomeKey = QStringLiteral("ui/welcomeDone");
    static inline const QString kGroupName = QStringLiteral("input");

    // `store` is not owned; it must outlive this object.
    explicit SetupCheck(QSettings *store, QObject *parent = nullptr);

    bool welcomeDone() const;
    void setWelcomeDone(bool done);

    GroupState groupState() const { return m_groupState; }
    bool inInputGroup() const { return m_groupState == InGroup; }
    QString userName() const { return m_userName; }
    QString groupCommand() const;
    QString serviceCommand() const;

    // Re-reads the group facts (e.g. when the welcome window regains focus).
    Q_INVOKABLE void refresh();

    // Pure decision used by refresh(); unit-tested.
    // processGids: the session's effective + supplementary groups.
    // accountGids: the groups the account database lists for the user.
    static GroupState evaluate(bool groupExists, unsigned inputGid, const QList<unsigned> &processGids,
                               const QList<unsigned> &accountGids);

Q_SIGNALS:
    void welcomeDoneChanged();
    void changed();

private:
    QSettings *m_store;
    GroupState m_groupState = NoGroup;
    QString m_userName;
};
