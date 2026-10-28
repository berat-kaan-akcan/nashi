#pragma once

#include <QAbstractListModel>
#include <QJsonObject>
#include <QStringList>
#include <QtQmlIntegration/qqmlintegration.h>

// External tool availability from `nashi-cli deps --json`.
class DepsModel : public QAbstractListModel
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Provided by the application")

    Q_PROPERTY(bool loading READ loading NOTIFY loadingChanged)
    Q_PROPERTY(QString error READ error NOTIFY changed)
    Q_PROPERTY(int missingCount READ missingCount NOTIFY changed)
    Q_PROPERTY(QStringList missingPackages READ missingPackages NOTIFY changed)
    Q_PROPERTY(QString installCommand READ installCommand NOTIFY changed)
    Q_PROPERTY(QString requiredInstallCommand READ requiredInstallCommand NOTIFY changed)

public:
    enum Roles { IdRole = Qt::UserRole + 1, AvailableRole, VersionRole, PathRole, PackageRole, OptionalRole, UsedForRole };

    struct Tool
    {
        QString id;
        bool available = false;
        QString version;
        QString path;
        QString package;
        bool optional = false;
        QStringList usedFor;
    };

    explicit DepsModel(QObject *parent = nullptr);

    static QList<Tool> parse(const QJsonObject &deps);
    static QString pacmanCommand(const QStringList &packages);

    void setProgram(const QString &program) { m_program = program; }
    Q_INVOKABLE void refresh();
    void setData(const QJsonObject &deps);

    int rowCount(const QModelIndex &parent = QModelIndex()) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    bool loading() const { return m_loading; }
    QString error() const { return m_error; }
    int missingCount() const;
    QStringList missingPackages(bool includeOptional = true) const;
    QString installCommand() const { return pacmanCommand(missingPackages(true)); }
    QString requiredInstallCommand() const { return pacmanCommand(missingPackages(false)); }

Q_SIGNALS:
    void changed();
    void loadingChanged();

private:
    QString m_program;
    QList<Tool> m_tools;
    bool m_loading = false;
    QString m_error;
};
