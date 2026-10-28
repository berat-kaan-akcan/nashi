#pragma once

#include <QAbstractListModel>
#include <QJsonObject>
#include <QList>
#include <QStringList>
#include <QtQmlIntegration/qqmlintegration.h>

struct WheelItem
{
    QString id;
    QString kind; // "convert" | "tool" | "action"
    QString title;
    QString icon;
    bool available = true;
    QStringList missing;
};

// Wheel items for one mode, built from `nashi-cli wheel --json`.
class WheelModel : public QAbstractListModel
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Provided by the application")

    Q_PROPERTY(QString mode READ mode WRITE setMode NOTIFY modeChanged)
    Q_PROPERTY(QString language READ language WRITE setLanguage NOTIFY languageChanged)
    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)
    Q_PROPERTY(bool hasData READ hasData NOTIFY countChanged)

public:
    enum Roles { IdRole = Qt::UserRole + 1, KindRole, TitleRole, IconRole, AvailableRole, MissingRole };

    explicit WheelModel(QObject *parent = nullptr);

    // Pure builder (unit-tested): picks the list for `mode` ("convert",
    // "tools", "actions"), drops tools with needs_editor, localises titles.
    static QList<WheelItem> buildItems(const QJsonObject &wheel, const QString &mode, const QString &language);
    // Localised title from either a plain string or an {"en","tr"} object.
    static QString localizedTitle(const QJsonValue &title, const QString &language);
    static QString normalizeMode(const QString &mode);

    int rowCount(const QModelIndex &parent = QModelIndex()) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    QString mode() const { return m_mode; }
    void setMode(const QString &mode);
    QString language() const { return m_language; }
    void setLanguage(const QString &language);
    bool hasData() const { return m_hasData; }

    void setWheel(const QJsonObject &wheel);
    void clear();
    const QList<WheelItem> &items() const { return m_items; }
    Q_INVOKABLE QVariantMap get(int row) const;
    Q_INVOKABLE int countFor(const QString &mode) const;

Q_SIGNALS:
    void modeChanged();
    void languageChanged();
    void countChanged();

private:
    void rebuild();

    QJsonObject m_wheel;
    bool m_hasData = false;
    QString m_mode = QStringLiteral("convert");
    QString m_language = QStringLiteral("en");
    QList<WheelItem> m_items;
};
