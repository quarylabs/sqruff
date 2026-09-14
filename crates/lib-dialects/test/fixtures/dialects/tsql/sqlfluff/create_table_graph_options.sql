CREATE TABLE dbo.Related (
    CONNECTION (dbo.Person TO dbo.Person, dbo.Person TO dbo.Company,)
        ON UPDATE NO ACTION ON DELETE CASCADE
) AS EDGE ON [PRIMARY];

CREATE TABLE dbo.People (
    id INT,
    CONSTRAINT pk_people PRIMARY KEY (id) WITH (PAD_INDEX = ON, FILLFACTOR = 80) ON [PRIMARY],
    CONSTRAINT ck_people CHECK NOT FOR REPLICATION (id > 0),
    CONSTRAINT fk_people FOREIGN KEY (id) REFERENCES dbo.Person (id)
) AS NODE;

CREATE TABLE dbo.IndexedPeople (
    id INT,
    INDEX ix_people NONCLUSTERED (id) WITH (ALLOW_PAGE_LOCKS = ON)
        ON ps_people(id) FILESTREAM_ON [PRIMARY]
) AS NODE ON ps_people(id);

CREATE TABLE dbo.OnlinePeople (
    id INT,
    INDEX ix_online (id) WITH (
        ONLINE = ON (WAIT_AT_LOW_PRIORITY (MAX_DURATION = 5 MINUTES, ABORT_AFTER_WAIT = SELF)),
        COMPRESSION_DELAY = 1 MINUTES,
        DATA_COMPRESSION = PAGE ON PARTITIONS (1 TO 3)
    )
) AS NODE;
