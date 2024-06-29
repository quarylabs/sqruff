pub mod ansi;
pub mod ansi_keywords;
pub mod bigquery;
pub mod bigquery_keywords;
pub mod clickhouse;
pub mod clickhouse_keywords;
pub mod postgres;
pub mod postgres_keywords;
pub mod snowflake;
pub mod snowflake_keywords;

#[derive(Debug, PartialEq, Eq, Clone, Copy, strum_macros::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum SyntaxKind {
    File,
    ColumnReference,
    ObjectReference,
    Expression,
    WildcardIdentifier,
    Function,
    HavingClause,
    PathSegment,
    LimitClause,
    CubeRollupClause,
    #[strum(serialize = "function_name")]
    RollupFunctionName,
    #[strum(serialize = "function_name")]
    CubeFunctionName,
    GroupingSetsClause,
    GroupingExpressionList,
    SetClause,
    FetchClause,
    FunctionDefinition,
    AlterSequenceOptionsSegment,
    RoleReference,
    TablespaceReference,
    ExtensionReference,
    TagReference,
    ColumnDefinition,
    ColumnConstraintSegment,
    CommentClause,
    TableEndClause,
    MergeMatch,
    MergeWhenNotMatchedClause,
    MergeInsertClause,
    MergeUpdateClause,
    MergeDeleteClause,
    SetClauseList,
    TableReference,
    #[strum(serialize = "table_reference")]
    SchemaReference,
    #[strum(serialize = "identifier_list")]
    SingleIdentifierList,
    #[strum(serialize = "groupby_clause")]
    GroupByClause,
    FrameClause,
    WithCompoundStatement,
    CommonTableExpression,
    CTEColumnList,
    #[strum(serialize = "column_reference")]
    SequenceReference,
    TriggerReference,
    TableConstraint,
    JoinOnCondition,
    DatabaseReference,
    #[strum(serialize = "database_reference")]
    IndexReference,
    CollationReference,
    OverClause,
    NamedWindow,
    WindowSpecification,
    #[strum(serialize = "partitionby_clause")]
    PartitionByClause,
    JoinClause,
    DropTriggerStatement,
    SampleExpression,
    TableExpression,
    CreateTriggerStatement,
    DropModelStatement,
    DescribeStatement,
    UseStatement,
    ExplainStatement,
    CreateSequenceStatement,
    CreateSequenceOptionsSegment,
    AlterSequenceStatement,
    DropSequenceStatement,
    DropCastStatement,
    CreateFunctionStatement,
    DropFunctionStatement,
    CreateModelStatement,
    CreateViewStatement,
    DeleteStatement,
    UpdateStatement,
    CreateCastStatement,
    CreateRoleStatement,
    DropRoleStatement,
    AlterTableStatement,
    CreateSchemaStatement,
    SetSchemaStatement,
    DropSchemaStatement,
    DropTypeStatement,
    CreateDatabaseStatement,
    DropDatabaseStatement,
    FunctionParameterList,
    CreateIndexStatement,
    DropIndexStatement,
    CreateTableStatement,
    AccessStatement,
    InsertStatement,
    TransactionStatement,
    DropTableStatement,
    DropViewStatement,
    CreateUserStatement,
    DropUserStatement,
    #[strum(serialize = "comparison_operator")]
    NotEqualTo,
    #[strum(serialize = "binary_operator")]
    Concat,
    ArrayExpression,
    LocalAlias,
    MergeStatement,
    IndexColumnDefinition,
    #[strum(serialize = "comparison_operator")]
    BitwiseAnd,
    #[strum(serialize = "comparison_operator")]
    BitwiseOr,
    #[strum(serialize = "comparison_operator")]
    BitwiseLShift,
    #[strum(serialize = "comparison_operator")]
    BitwiseRShift,
    #[strum(serialize = "comparison_operator")]
    LessThan,
    #[strum(serialize = "comparison_operator")]
    GreaterThanOrEqualTo,
    #[strum(serialize = "comparison_operator")]
    LessThanOrEqualTo,
    #[strum(serialize = "comparison_operator")]
    Equals,
    #[strum(serialize = "comparison_operator")]
    GreaterThan,
    #[strum(serialize = "numeric_literal")]
    QualifiedNumericLiteral,
    AggregateOrderByClause,
    FunctionName,
    CaseExpression,
    WhenClause,
    ElseClause,
    WhereClause,
    SetOperator,
    ValuesClause,
    EmptyStructLiteral,
    ObjectLiteral,
    ObjectLiteralElement,
    TimeZoneGrammar,
    BracketedArguments,
    DataType,
    AliasExpression,
    #[strum(serialize = "cast_expression")]
    ShorthandCast,
    ArrayAccessor,
    ArrayLiteral,
    TypedArrayLiteral,
    StructType,
    StructLiteral,
    TypedStructLiteral,
    IntervalExpression,
    ArrayType,
    SizedArrayType,
    SelectStatement,
    OverlapsClause,
    SelectClause,
    Statement,
    WithNoSchemaBindingClause,
    WithDataClause,
    SetExpression,
    FromClause,
    EmptyStructLiteralBrackets,
    WildcardExpression,
    #[strum(serialize = "orderby_clause")]
    OrderByClause,
    TruncateStatement,
    FromExpression,
    FromExpressionElement,
    SelectClauseModifier,
    NamedWindowExpression,
    SelectClauseElement,
    #[strum(serialize = "set_operator")]
    SetOperatorSegment,
    QualifyClause,
    MultiStatementSegment,
    AssertStatement,
    ForInStatements,
    ForInStatement,
    RepeatStatements,
    RepeatStatement,
    IfStatements,
    IfStatement,
    LoopStatements,
    LoopStatement,
    WhileStatements,
    WhileStatement,
    DatePartWeek,
    SelectExceptClause,
    SelectReplaceClause,
    StructTypeSchema,
    Tuple,
    NamedArgument,
    DeclareSegment,
    SetSegment,
    PartitionBySegment,
    ClusterBySegment,
    OptionsSegment,
    CreateExternalTableStatement,
    AlterViewStatement,
    CreateMaterializedViewStatement,
    AlterMaterializedViewSetOptionsStatement,
    DropMaterializedViewStatement,
    ParameterizedExpression,
    PivotForClause,
    FromPivotExpression,
    FromUnpivotExpression,
    NotMatchedByTargetClause,
    MergeWhenMatchedClause,
    ProcedureName,
    ExportStatement,
    ProcedureParameterList,
    ProcedureStatements,
    CallStatement,
    ReturnStatement,
    BreakStatement,
    LeaveStatement,
    ContinueStatement,
    RaiseStatement,
    PsqlVariable,
    ComparisonOperator,
    #[strum(serialize = "datetime_type_identifier")]
    DateTimeTypeIdentifier,
    #[strum(serialize = "datetime_literal")]
    DateTimeLiteral,
    IndexAccessMethod,
    OperatorClassReference,
    DefinitionParameter,
    DefinitionParameters,
    RelationOption,
    RelationOptions,
    AlterFunctionActionSegment,
    AlterProcedureActionSegment,
    AlterProcedureStatement,
    DropProcedureStatement,
    WktGeometryType,
    IntoClause,
    ForClause,
    GroupbyClause,
    AlterRoleStatement,
    ExplainOption,
    CreateTableAsStatement,
    AlterPublicationStatement,
    CreatePublicationStatement,
    PublicationObjects,
    PublicationTable,
    PublicationReference,
    DropExtensionStatement,
    CreateExtensionStatement,
    VersionIdentifier,
    AlterTableActionSegment,
    DropPublicationStatement,
    AlterMaterializedViewStatement,
    AlterMaterializedViewActionSegment,
    RefreshMaterializedViewStatement,
    WithCheckOption,
    AlterPolicyStatement,
    AlterDatabaseStatement,
    VacuumStatement,
    LikeOptionSegment,
    PartitionBoundSpec,
    IndexParameters,
    ReferentialActionSegment,
    IndexElement,
    ExclusionConstraintElement,
    AlterDefaultPrivilegesStatement,
    AlterDefaultPrivilegesObjectPrivilege,
    AlterDefaultPrivilegesSchemaObject,
    AlterDefaultPrivilegesToFromRoles,
    AlterDefaultPrivilegesGrant,
    DropOwnedStatement,
    #[strum(serialize = "comment_clause")]
    CommentOnStatementSegment,
    ReassignOwnedStatement,
    IndexElementOptions,
    AlterDefaultPrivilegesRevoke,
    AlterIndexStatement,
    ReindexStatementSegment,
    AnalyzeStatement,
    AlterTrigger,
    #[strum(serialize = "alias_expression")]
    AsAliasExpression,
    OperationClassReference,
    ConflictAction,
    ConflictTarget,
    SetStatement,
    CreatePolicyStatement,
    CreateDomainStatement,
    AlterDomainStatement,
    DropDomainStatement,
    DropPolicyStatement,
    LoadStatement,
    ResetStatement,
    ListenStatement,
    NotifyStatement,
    UnlistenStatement,
    ClusterStatement,
    LanguageClause,
    DoStatement,
    CreateUserMappingStatement,
    ImportForeignSchemaStatement,
    CreateServerStatement,
    CreateCollationStatement,
    AlterTypeStatement,
    CreateTypeStatement,
    LockTableStatement,
    CopyStatement,
    DiscardStatement,
    AlterSchemaStatement,
    ServerReference,
    ArrayJoinClause,
    TableEngineFunction,
    OnClusterClause,
    Engine,
    EngineFunction,
    DatabaseEngine,
    ColumnTtlSegment,
    TableTtlSegment,
    DropDictionaryStatement,
    DropQuotaStatement,
    DropSettingProfileStatement,
    SystemMergesSegment,
    SystemTtlMergesSegment,
    SystemMovesSegment,
    SystemReplicaSegment,
    SystemFilesystemSegment,
    SystemReplicatedSegment,
    SystemReplicationSegment,
    SystemFetchesSegment,
    SystemDistributedSegment,
    SystemModelSegment,
    SystemFileSegment,
    SystemUnfreezeSegment,
    SystemStatement,
    ConnectbyClause,
    CallSegment,
    WithingroupClause,
    PatternExpression,
    MatchRecognizeClause,
    ChangesClause,
    FromAtExpression,
    FromBeforeExpression,
    SnowflakeKeywordExpression,
    SemiStructuredExpression,
    SelectExcludeClause,
    SelectRenameClause,
    AlterTableTableColumnAction,
    AlterTableClusteringAction,
    AlterTableConstraintAction,
    AlterWarehouseStatement,
    AlterShareStatement,
    AlterStorageIntegrationStatement,
    AlterExternalTableStatement,
    CommentEqualsClause,
    TagBracketedEquals,
    TagEquals,
    CreateCloneStatement,
    CreateDatabaseFromShareStatement,
    CreateProcedureStatement,
    ScriptingBlockStatement,
    ScriptingLetStatement,
    AlterFunctionStatement,
    CreateExternalFunctionStatement,
    WarehouseObjectProperties,
    ConstraintPropertiesSegment,
    CopyOptions,
    SchemaObjectProperties,
    CreateTaskStatement,
    SnowflakeTaskExpressionSegment,
    CreateStatement,
    CreateFileFormatSegment,
    AlterFileFormatSegment,
    CsvFileFormatTypeParameters,
    JsonFileFormatTypeParameters,
    AvroFileFormatTypeParameters,
    OrcFileFormatTypeParameters,
    ParquetFileFormatTypeParameters,
    XmlFileFormatTypeParameters,
    AlterPipeSegment,
    FileFormatSegment,
    FormatTypeOptions,
    CopyIntoLocationStatement,
    CopyIntoTableStatement,
    StorageLocation,
    StageParameters,
    S3ExternalStageParameters,
    GcsExternalStageParameters,
    AzureBlobStorageExternalStageParameters,
    CreateStageStatement,
    AlterStageStatement,
    CreateStreamStatement,
    AlterStreamStatement,
    ShowStatement,
    AlterUserStatement,
    AlterSessionStatement,
    AlterSessionSetStatement,
    AlterSessionUnsetClause,
    AlterTaskStatement,
    AlterTaskSpecialSetClause,
    AlterTaskSetClause,
    AlterTaskUnsetClause,
    ExecuteTaskClause,
    UndropStatement,
    CommentStatement,
    DropExternalTableStatement,
    ListStatement,
    GetStatement,
    PutStatement,
    RemoveStatement,
    CastExpression,
    DropObjectStatement,
    UnsetStatement,
}

impl SyntaxKind {
    pub fn as_str(&self) -> &'static str {
        self.into()
    }
}
