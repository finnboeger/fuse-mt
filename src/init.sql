CREATE TABLE [Cover] (
    [ID] INTEGER  NOT NULL PRIMARY KEY AUTOINCREMENT,
    [Filename] TEXT  UNIQUE NOT NULL,
    [Date] INTEGER  NOT NULL,
    [Width] INTEGER  NOT NULL,
    [Height] INTEGER  NOT NULL
);
CREATE INDEX [Cover_Filename_IDX] ON [Cover]([Filename]  ASC);

CREATE TABLE [CoverThumbnail] (
    [ID] INTEGER  NOT NULL PRIMARY KEY,
    [Format] INTEGER  NOT NULL,
    [Width] INTEGER  NOT NULL,
    [Height] INTEGER  NOT NULL,
    [Data] BLOB  NULL
);

PRAGMA user_version = 1;
