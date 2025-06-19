WITH OrderDeliverydateCriticalCTE AS (
	SELECT
		OrderID = Orders.I_ORDERS_P
		,CurrentState = b_deliverydatecritical
		,CalculatedState = (
			CASE WHEN EXISTS (
				SELECT
					1
				FROM
					sao.ORDERPOS_P														AS OrderPositions	WITH(READUNCOMMITTED)
					INNER JOIN sao.NegSoft_ERP_SalesOrderPosition2PurchaseOrderPosition	AS sop2pop			WITH(READUNCOMMITTED) ON sop2pop.i_orderpos_id = OrderPositions.I_ORDERPOS_P AND sop2pop.dt_deleted IS NULL
				WHERE
					OrderPositions.I_ORDERS_P = Orders.I_ORDERS_P
					AND OrderPositions.DT_DELETED IS NULL
					AND NxOrder.i_deliverydatetype_id IN (1,2) --> 1: bis zum; 2: am
					AND DATEDIFF(
						DAY,
						(
							-- calc potentially customer deliverydate
							SELECT
								EndDate
							FROM
								sao.negsoft_tblfct_calculateworkingdays(
									-- get max purchase deliverydate
									(
										SELECT
											MAX(sop2popPurOrdPos.D_DELIVERYDATE)
										FROM
											sao.NegSoft_ERP_SalesOrderPosition2PurchaseOrderPosition	AS sop2pop			WITH(NOLOCK)
											INNER JOIN sao.ORDERPOS_P									AS sop2popOrdPos	WITH(NOLOCK) ON sop2popOrdPos.I_ORDERPOS_P = SOP2POP.i_orderpos_id AND sop2popOrdPos.DT_DELETED IS NULL
											INNER JOIN sao.ORDERS_P										AS sop2popOrd		WITH(NOLOCK) ON sop2popOrd.I_ORDERS_P = sop2popOrdPos.I_ORDERS_P AND sop2popOrd.DT_DELETED IS NULL
											INNER JOIN sao.PURORDPOS_P									AS sop2popPurOrdPos	WITH(NOLOCK) ON sop2popPurOrdPos.I_PURORDPOS_P = SOP2POP.i_purchaseorderpos_id AND sop2popPurOrdPos.DT_DELETED IS NULL
											INNER JOIN sao.PURORDER_p									AS sop2popPurOrd	WITH(NOLOCK) ON sop2popPurOrd.I_PURORDER_P = sop2popPurOrdPos.I_PURORDER_P AND sop2popPurOrd.DT_DELETED IS NULL
											LEFT OUTER JOIN (
												SELECT
													Sop2PopId = i_sop2pop_id,
													ItemQuantity = SUM(i_itemquantity)
												FROM
													sao.NegSoft_ERP_SalesOrderPosition2PurchaseOrderPositionLogistic WITH(NOLOCK)
												WHERE
													dt_deleted IS NULL
												GROUP BY
													i_sop2pop_id
											) AS sop2popLogistic ON sop2popLogistic.Sop2PopId = SOP2POP.id
										WHERE
											sop2pop.dt_deleted IS NULL
											AND sop2popOrd.i_orders_p = Orders.i_orders_p
											AND COALESCE(sop2popLogistic.ItemQuantity, 0) < SOP2POP.f_quantity
											AND COALESCE(sop2popPurOrdPos.N_ITEMSTOREDQUANT, 0) < sop2popPurOrdPos.N_ITEMQUANTITY
											AND sop2popOrd.N_STATE < 16384
									),
									NULL,
									CASE WHEN (ISNULL((
										SELECT
											SUM(SalesOrderPosition2PurchaseOrderPosition.f_quantity)
										FROM
											sao.NegSoft_ERP_SalesOrderPosition2PurchaseOrderPosition	AS SalesOrderPosition2PurchaseOrderPosition	WITH(NOLOCK)
											INNER JOIN sao.ORDERPOS_P									AS OrdPos									WITH(NOLOCK) ON OrdPos.I_ORDERPOS_P = SalesOrderPosition2PurchaseOrderPosition.i_orderpos_id AND OrdPos.DT_DELETED IS NULL
											LEFT OUTER JOIN sao.PURORDPOS_P								AS PurchaseOrderPosition					WITH(NOLOCK) ON PurchaseOrderPosition.I_PURORDPOS_P = SalesOrderPosition2PurchaseOrderPosition.i_purchaseorderpos_id
											LEFT OUTER JOIN sao.PURORDER_P								AS PurchaseOrder							WITH(NOLOCK) ON PurchaseOrder.I_PURORDER_P = PurchaseOrderPosition.I_PURORDER_P
											LEFT OUTER JOIN sao.NegSoft_ERP_PurchaseOrder				AS NXPurchaseOrder							WITH(NOLOCK) ON NXPurchaseOrder.id = PurchaseOrder.I_PURORDER_P
										WHERE
											OrdPos.I_ORDERS_P = Orders.I_ORDERS_P
											AND SalesOrderPosition2PurchaseOrderPosition.dt_deleted IS NULL
											AND PurchaseOrderPosition.DT_DELETED IS NULL
											AND PurchaseOrder.B_CANCEL = 0
											AND (PurchaseOrderPosition.N_ITEMSTOREDQUANT > 0 OR PurchaseOrder.N_STATE < 16384)
											AND PurchaseOrder.DT_DELETED IS NULL
											AND NXPurchaseOrder.b_dropshipment = 1
									), 0)) > 0 THEN 0 ELSE 2 END --> add MCL process time unless it's a dropshipment
									+ (
										CASE WHEN (
											(
												SELECT
													COUNT(*)
												FROM
													sao.ORDERPOS_P AS PositionMain WITH(READUNCOMMITTED)
													INNER JOIN sao.NegSoft_ERP_SalesOrderPosition AS NxPositionReference WITH(READUNCOMMITTED) ON NxPositionReference.i_installedintoposition_id = PositionMain.I_ORDERPOS_P
													INNER JOIN sao.ORDERPOS_P AS PositionReference WITH(READUNCOMMITTED) ON PositionReference.I_ORDERPOS_P = NxPositionReference.id AND PositionReference.DT_DELETED IS NULL
												WHERE
													PositionMain.DT_DELETED IS NULL
													AND PositionMain.I_ORDERS_P = OrderPositions.I_ORDERS_P
													AND OrderPositions.I_ORDERPOS_P IN (PositionMain.I_ORDERPOS_P, PositionReference.I_ORDERPOS_P)
											) + (
												SELECT
													COUNT(*)
												FROM
													sao.NegSoft_ERP_SalesOrderPositionReference		AS op2ref		WITH(NOLOCK)
													INNER JOIN sao.ORDERPOS_P						AS Position		WITH(NOLOCK) ON Position.I_ORDERPOS_P = op2ref.i_position_id AND Position.DT_DELETED IS NULL
													INNER JOIN sao.ORDERPOS_P						AS PositionRef	WITH(NOLOCK) ON PositionRef.I_ORDERPOS_P = op2ref.i_positionref_id AND PositionRef.DT_DELETED IS NULL
												WHERE
													OrderPositions.I_ORDERPOS_P IN (op2ref.i_position_id, op2ref.i_positionref_id)
													AND op2ref.i_referencetype_id = 1
													AND OrderPositions.I_ORDERS_P = Orders.I_ORDERS_P
													AND op2ref.dt_deleted IS NULL
											) > 0
										) THEN 1 ELSE 0 END
									)		--> if factory + 1 day MCL process time
									+ 1,	--> current day
									'BW'	--> LocationSate
								)
						),
						NxOrder.dt_deliverydate
					) < 0
				)
				THEN 1
				ELSE 0
			END
		)
	FROM
		sao.ORDERS_P AS Orders WITH(READUNCOMMITTED)
		INNER JOIN sao.NegSoft_ERP_SalesOrder AS NxOrder WITH(READUNCOMMITTED) ON NxOrder.id = Orders.I_ORDERS_P
	WHERE
		Orders.N_STATE <= 32
		AND (
			NxOrder.i_deliverydatetype_id IN (1,2) --> 1: bis zum; 2: am
			OR NxOrder.dt_communicateddeliverydate IS NOT NULL
		)
		AND Orders.DT_DELETED IS NULL
)
UPDATE
	NxOrder
SET
	b_deliverydatecritical = OrderDeliverydateCriticalCTE.CalculatedState
FROM
	OrderDeliverydateCriticalCTE
	INNER JOIN sao.NegSoft_ERP_SalesOrder AS NxOrder WITH(READUNCOMMITTED) ON NxOrder.id = OrderDeliverydateCriticalCTE.OrderID
WHERE
	OrderDeliverydateCriticalCTE.CalculatedState != OrderDeliverydateCriticalCTE.CurrentState;
